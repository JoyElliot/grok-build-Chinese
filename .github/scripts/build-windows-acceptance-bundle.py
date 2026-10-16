"""Assemble the offline, preview-only Windows acceptance download.

Only CI needs Python. The download contains the exact validated native and
embedded-GNU ZIPs, a small explicitly allowed PowerShell kit and provenance.
Checksums detect corruption; they are not a signature or a release approval.
"""
import argparse
import importlib.util
import json
from pathlib import Path
import re
import shutil
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[2]
KIT = ROOT / "packaging/windows/acceptance"
KIT_FILES = ("Run-WindowsAcceptance.ps1", "Start-IsolatedTui.ps1", "README.zh-CN.md")
PROVENANCE = "ACCEPTANCE-PROVENANCE.json"
MANIFEST = "SHA256SUMS.txt"
MAX_KIT_FILE_BYTES = 1024 * 1024
SCOPE = {
    "preview_only": True,
    "automated_checks": ["bundle-integrity", "native-cli", "embedded-offline-migration",
                         "portable-install", "install-marker", "restart-idempotence",
                         "synthetic-data-and-path-preservation"],
    "excluded_checks": ["interactive-tui-rendering", "login-and-account", "online-update",
                        "release-approval", "windows-version-matrix", "historical-client-download",
                        "two-version-update", "downgrade", "antivirus-low-disk-interruption"],
}

spec = importlib.util.spec_from_file_location(
    "acceptance_embedded_msvc_bundle", Path(__file__).with_name("embedded-msvc-bundle.py"))
bundle = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bundle)


def validate_identity(commit, repository, run_id, run_attempt):
    if not re.fullmatch(r"[0-9a-f]{40}", commit):
        raise ValueError("commit must be a complete lowercase Git SHA-1")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("repository must be a GitHub owner/repository")
    if any(part in (".", "..") for part in repository.split("/")):
        raise ValueError("invalid repository name")
    if any(not isinstance(value, str) or not re.fullmatch(r"[1-9][0-9]{0,19}", value)
           for value in (run_id, run_attempt)):
        raise ValueError("run identity must contain positive decimal strings")


def copy_regular(source, destination, maximum):
    source = Path(source)
    if source.is_symlink() or not source.is_file() or not 0 < source.stat().st_size <= maximum:
        raise ValueError("input must be a bounded, nonempty regular file: " + str(source))
    with source.open("rb") as reader, destination.open("xb") as writer:
        shutil.copyfileobj(reader, writer, bundle.CHUNK)
    if not 0 < destination.stat().st_size <= maximum:
        raise ValueError("copied input exceeded its size bound")


def archive_descriptor(path, validated, target):
    return dict(file=path.name, sha256=validated["archive_sha256"],
                size=validated["archive_size"], target=target)


def build_bundle(output, native_archive, embedded_gnu_archive, version, commit,
                 repository, run_id, run_attempt, *, kit_directory=KIT):
    """Validate staged bytes before atomically publishing a new flat directory."""
    bundle.canonical_version(version)
    validate_identity(commit, repository, run_id, run_attempt)
    output = Path(output)
    if output.exists() or output.is_symlink():
        raise FileExistsError("refusing to overwrite an existing acceptance bundle")
    if not output.parent.is_dir():
        raise ValueError("acceptance output parent must already exist")
    native_name = f"grok-zh-{version}-windows-x86_64-msvc.zip"
    gnu_name = f"grok-zh-{version}-windows-x86_64-gnu.zip"
    if Path(native_archive).name != native_name or Path(embedded_gnu_archive).name != gnu_name:
        raise ValueError("input archive names must match the canonical version and target")
    with tempfile.TemporaryDirectory(prefix=".grok-acceptance-", dir=output.parent) as temporary:
        staged = Path(temporary) / "bundle"
        staged.mkdir()
        native_path, gnu_path = staged / native_name, staged / gnu_name
        copy_regular(native_archive, native_path, bundle.MAX_ARCHIVE_BYTES)
        copy_regular(embedded_gnu_archive, gnu_path, bundle.MAX_ARCHIVE_BYTES)
        # Validate the actual copies to be shipped, including every nested hash,
        # footer bounds, PE identity, legacy file allowlist and native protocol.
        native = bundle.validate_native_archive(native_path, version)
        embedded = bundle.validate_compat_archive(gnu_path, version)
        metadata = embedded["embedded_runtime"]
        if (metadata["archive_sha256"] != native["archive_sha256"] or
                metadata["archive_size"] != native["archive_size"]):
            raise ValueError("embedded payload is not the exact supplied native ZIP")
        with zipfile.ZipFile(native_path) as zipped:
            info = zipped.read(native["root"] + "/BUILD-INFO.txt").decode("utf-8-sig")
        for key, value in (("Commit", commit), ("Repository", repository)):
            actual = re.findall(r"^" + key + r":[ \t]*(.*?)[ \t]*\r?$", info, re.M)
            if actual != [value]:
                raise ValueError("native BUILD-INFO " + key + " does not match provenance")
        for name in KIT_FILES:
            copy_regular(Path(kit_directory) / name, staged / name, MAX_KIT_FILE_BYTES)
        provenance = dict(schema=1, version=version, commit=commit, repository=repository,
                          run_id=run_id, run_attempt=run_attempt,
                          run_url=f"https://github.com/{repository}/actions/runs/{run_id}/attempts/{run_attempt}",
                          scope=SCOPE,
                          archives=dict(
                              native=archive_descriptor(native_path, native, bundle.TARGET),
                              embedded_gnu=dict(
                                  archive_descriptor(gnu_path, embedded["outer"], "x86_64-pc-windows-gnu"),
                                  embedded_native_sha256=metadata["archive_sha256"],
                                  embedded_native_size=metadata["archive_size"])))
        (staged / PROVENANCE).write_text(json.dumps(provenance, indent=2, ensure_ascii=False) + "\n",
                                         encoding="utf-8", newline="\n")
        # The manifest covers all shipped files, including provenance and scripts,
        # except itself. Never recursively copy a directory or CI/compiler files.
        names = sorted((native_name, gnu_name, PROVENANCE, *KIT_FILES))
        (staged / MANIFEST).write_text("".join(
            f"{bundle.digest_file(staged / name)}  {name}\n" for name in names),
            encoding="ascii", newline="\n")
        staged.rename(output)
    return provenance


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--native-archive", required=True, type=Path)
    parser.add_argument("--embedded-gnu-archive", required=True, type=Path)
    for field in ("version", "commit", "repository", "run-id", "run-attempt"):
        parser.add_argument("--" + field, required=True)
    args = parser.parse_args()
    result = build_bundle(args.output, args.native_archive, args.embedded_gnu_archive,
                          args.version, args.commit, args.repository, args.run_id, args.run_attempt)
    print(json.dumps(result, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
