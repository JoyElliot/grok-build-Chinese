"""Plan the default-off Windows x64 transition.

Only checked-in policy can activate a distribution. Direct mode ships an unchanged
native EXE with an explicit legacy transport descriptor, retaining the native
archive for older MSVC clients. Embedded mode publishes one
legacy-named container; its native archive is an internal build/provenance input.
Native-only retirement requires an authenticated immutable final wrapper. This
script never creates or changes releases, tags, or repository settings.
"""

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import tempfile
import urllib.request
from urllib.parse import urlsplit

ROOT = Path(__file__).resolve().parents[2]
REPOSITORY = "JoyElliot/grok-build-Chinese"
MAX_WRAPPER_BYTES = 512 * 1024 * 1024


def bootstrap_builder():
    spec = importlib.util.spec_from_file_location(
        "embedded_migration_builder", ROOT / ".github/scripts/build-windows-embedded-bootstrap.py"
    )
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def stable_version(version):
    if not isinstance(version, str) or len(version) > 64:
        raise ValueError("expected a canonical stable version")
    match = re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version)
    if not match or any(int(n) > 2**64 - 1 for n in match.groups()):
        raise ValueError("expected a canonical stable version")
    return tuple(map(int, match.groups()))


def validate_wrapper_pin(pin):
    if not isinstance(pin, dict) or set(pin) != {"version", "size", "sha256"}:
        raise ValueError("native retirement requires a reviewed final legacy wrapper pin")
    if (stable_version(pin["version"]) <= (1, 0, 45) or
            type(pin["size"]) is not int or not 0 < pin["size"] <= MAX_WRAPPER_BYTES or
            not isinstance(pin["sha256"], str) or not re.fullmatch(r"[0-9a-f]{64}", pin["sha256"])):
        raise ValueError("invalid final legacy wrapper pin")
    return pin


def plan_release(policy, version, tag, repository):
    config = policy.get("windows_x64_msvc", {
        "enabled": False, "distribution": "embedded", "final_legacy_wrapper": None})
    if (not isinstance(config, dict) or
            set(config) != {"enabled", "distribution", "final_legacy_wrapper"} or
            type(config["enabled"]) is not bool or config["distribution"] not in ("embedded", "direct", "native")):
        raise ValueError("windows_x64_msvc requires enabled, distribution and final_legacy_wrapper")
    enabled = config["enabled"]
    pin = config["final_legacy_wrapper"]
    if pin is not None:
        validate_wrapper_pin(pin)
    if enabled:
        if repository != REPOSITORY:
            raise ValueError("migration is restricted to the official repository")
        current = stable_version(version)
        if current <= (1, 0, 45) or tag != f"release-v{version}":
            raise ValueError("migration requires a stable release newer than preserved 1.0.45")
        if policy.get("publish_legacy_sha256") is not False:
            raise ValueError("migration requires the digest-only public asset policy")
        if config["distribution"] == "native":
            validate_wrapper_pin(pin)
            if stable_version(pin["version"]) >= current:
                raise ValueError("final legacy wrapper must precede the native-only release")
    distribution = config["distribution"] if enabled else "gnu"
    native = f"grok-zh-{version}-windows-x86_64-msvc.zip"
    legacy = f"grok-zh-{version}-windows-x86_64-gnu.zip"
    return {
        "windows_msvc_enabled": str(enabled).lower(),
        "windows_x64_distribution": distribution,
        "windows_public_archive_name": native if distribution == "native" else legacy,
        "msvc_package_name": f"grok-zh-{version}-windows-x86_64-msvc",
        "msvc_archive_name": native,
    }


def validate_wrapper_release(release, pin, repository):
    validate_wrapper_pin(pin)
    tag = f"release-v{pin['version']}"
    name = f"grok-zh-{pin['version']}-windows-x86_64-gnu.zip"
    if (release.get("tag_name") != tag or release.get("immutable") is not True or
            release.get("draft") is not False or release.get("prerelease") is not False):
        raise ValueError("final wrapper must remain in a published immutable stable release")
    assets = release.get("assets")
    if not isinstance(assets, list):
        raise ValueError("final wrapper release has no assets")
    matches = [a for a in assets if isinstance(a, dict) and a.get("name") == name]
    if len(matches) != 1:
        raise ValueError("final wrapper asset missing or ambiguous")
    asset = matches[0]
    expected_url = f"https://github.com/{repository}/releases/download/{tag}/{name}"
    if (asset.get("state") != "uploaded" or type(asset.get("size")) is not int or
            asset["size"] != pin["size"] or asset.get("digest") != f"sha256:{pin['sha256']}" or
            asset.get("browser_download_url") != expected_url):
        raise ValueError("final wrapper identity, digest, size or URL differs from reviewed pin")
    return expected_url


class SafeRedirectHandler(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, new_url):
        before, after = urlsplit(request.full_url), urlsplit(new_url)
        if after.scheme != "https" or after.username or after.password:
            raise ValueError("preflight refuses an unsafe redirect")
        if request.has_header("Authorization") and (before.hostname, before.port) != (after.hostname, after.port):
            raise ValueError("preflight refuses to forward credentials to another origin")
        return super().redirect_request(request, response, code, message, headers, new_url)


def fetch_bytes(url, limit, token=None):
    headers = {"User-Agent": "grok-zh-release-preflight"}
    if token:
        headers["Authorization"] = f"Bearer {token}"
    request = urllib.request.Request(url, headers=headers)
    with urllib.request.build_opener(SafeRedirectHandler()).open(request, timeout=120) as response:
        data = response.read(limit + 1)
    if len(data) > limit:
        raise ValueError("preflight response exceeds size limit")
    return data


def preflight(policy, version, tag, repository):
    plan = plan_release(policy, version, tag, repository)
    if plan["windows_x64_distribution"] != "native":
        # Embedded payloads are self-contained; neither building nor running
        # them depends on a helper release or any preflight network request.
        return plan
    pin = policy["windows_x64_msvc"]["final_legacy_wrapper"]
    url = f"https://api.github.com/repos/{repository}/releases/tags/release-v{pin['version']}"
    release = json.loads(fetch_bytes(url, 1024 * 1024, os.environ.get("GH_TOKEN")))
    asset_url = validate_wrapper_release(release, pin, repository)
    data = fetch_bytes(asset_url, pin["size"])  # Never send API token to CDN.
    if len(data) != pin["size"] or hashlib.sha256(data).hexdigest() != pin["sha256"]:
        raise ValueError("final wrapper bytes differ from reviewed pin")
    with tempfile.TemporaryDirectory() as temporary:
        archive = Path(temporary) / f"grok-zh-{pin['version']}-windows-x86_64-gnu.zip"
        archive.write_bytes(data)
        bootstrap_builder().validate_compat_archive(archive, pin["version"])
    return plan


def build_bridge(policy, version, tag, repository, output, payload, archive, cc):
    plan = preflight(policy, version, tag, repository)
    if plan["windows_x64_distribution"] != "embedded":
        raise ValueError("embedded distribution is not enabled")
    builder = bootstrap_builder()
    builder.validate_native_archive(archive, version)
    output.mkdir(parents=True, exist_ok=False)
    builder.build_launcher(output, version, archive, cc)
    return builder.build_compat_package(output, payload, version, release=True)


def build_direct(policy, version, tag, repository, output, archive):
    plan = preflight(policy, version, tag, repository)
    if plan["windows_x64_distribution"] != "direct":
        raise ValueError("direct distribution is not enabled")
    spec = importlib.util.spec_from_file_location(
        "direct_msvc_builder", ROOT / ".github/scripts/build-windows-direct-msvc.py")
    builder = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(builder)
    return builder.build_package(output, archive, version)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("plan", "preflight", "build-bridge", "build-direct"))
    parser.add_argument("--version", required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--repository", required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--payload-package", type=Path)
    parser.add_argument("--payload-archive", type=Path)
    parser.add_argument("--cc", default="x86_64-w64-mingw32-gcc")
    args = parser.parse_args()
    policy = json.loads((ROOT / ".github/release-policy.json").read_text(encoding="utf-8"))
    plan = plan_release(policy, args.version, args.tag, args.repository)
    if args.mode == "build-bridge":
        if args.output is None or args.payload_package is None or args.payload_archive is None:
            parser.error("build-bridge requires --output, --payload-package and --payload-archive")
        build_bridge(policy, args.version, args.tag, args.repository,
                     args.output, args.payload_package, args.payload_archive, args.cc)
    elif args.mode == "build-direct":
        if args.output is None or args.payload_archive is None:
            parser.error("build-direct requires --output and --payload-archive")
        build_direct(policy, args.version, args.tag, args.repository, args.output, args.payload_archive)
    elif args.mode == "preflight":
        preflight(policy, args.version, args.tag, args.repository)
    print(json.dumps(plan))


if __name__ == "__main__":
    main()
