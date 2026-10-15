"""Build one offline GNU-compatible transport carrying the full native MSVC ZIP.

Inputs are already-built authenticated package artifacts. No downloads, remote
release mutation, toolchain installation or payload-sized C arrays are used.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[2]
MIGRATION = ROOT / "packaging/windows/migration"


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


bundle = load_module("embedded_msvc_bundle", Path(__file__).with_name("embedded-msvc-bundle.py"))
legacy = load_module("windows_migration_builder", Path(__file__).with_name("build-windows-migration-bootstrap.py"))
LEGACY_FILES = legacy.LEGACY_FILES
validate_native_archive = bundle.validate_native_archive
validate_compat_archive = bundle.validate_compat_archive


def function_source(text, name):
    """Copy a top-level, checked-in function verbatim (including recovery fixes)."""
    matches = list(re.finditer(r"(?m)^function ([A-Za-z0-9-]+) \{", text))
    for index, match in enumerate(matches):
        if match.group(1) == name:
            end = matches[index + 1].start() if index + 1 < len(matches) else text.index("\nif ($MyInvocation", match.end())
            return text[match.start():end].rstrip() + "\n"
    raise ValueError(f"required offline function missing: {name}")


def offline_validation_bytes():
    # Intentionally omit all HTTP/release/download functions. This reuses the
    # established verifier and transactional recovery without a second trust path.
    online = (ROOT / "packaging/windows/Install-GrokZhOnline.ps1").read_text(encoding="utf-8-sig")
    migration = (MIGRATION / "Invoke-Bootstrap.ps1").read_text(encoding="utf-8-sig")
    names = ("Get-OnlineProperty", "ConvertFrom-OnlineUtf8", "ConvertTo-OnlineVersion", "Compare-OnlineVersion",
             "Assert-OnlinePathChain", "Resolve-OnlinePath", "Remove-OnlineOwnedTree", "Assert-OnlinePackageRelativePath",
             "Read-OnlinePackageProtocol", "Get-OnlineExecutableVersion", "Get-OnlineInstallMarker", "Invoke-OnlinePackageInstaller")
    text = "# Generated offline-only validation/recovery library.\n"
    text += "$script:OnlineTargetTriple = 'x86_64-pc-windows-msvc'\n"
    text += "$script:OnlineUtf8 = [Text.UTF8Encoding]::new($false, $true)\n"
    text += "\n".join(function_source(online, name) for name in names)
    text += "\n".join(function_source(migration, name) for name in ("Invoke-MigrationFileReplace", "Update-MigrationRuntime"))
    return b"\xef\xbb\xbf" + text.encode("utf-8")


def build_launcher(output, version, archive, cc="x86_64-w64-mingw32-gcc"):
    output, archive = Path(output), Path(archive)
    metadata = bundle.runtime_metadata(archive, version)
    output.mkdir(parents=True, exist_ok=True)
    header = f"#define BOOTSTRAP_VERSION {json.dumps(version)}\n"
    header += f"#define EMBEDDED_ARCHIVE_SIZE {metadata['archive_size']}ULL\n"
    header += f"#define EMBEDDED_ARCHIVE_SHA256 {json.dumps(metadata['archive_sha256'])}\n"
    header += legacy.c_array("bootstrap_script", legacy.script_bytes(MIGRATION / "Invoke-EmbeddedMigration.ps1"))
    header += legacy.c_array("validation_script", offline_validation_bytes())
    header += legacy.c_array("installer_script", legacy.script_bytes(ROOT / "packaging/windows/Install-GrokZh.ps1"))
    (output / "embedded_bootstrap_payload.h").write_text(header, encoding="ascii")
    prefix, exe = output / "launcher-prefix.exe", output / "grok-zh.exe"
    if prefix.exists() or exe.exists():
        raise FileExistsError("refusing to overwrite an existing launcher")
    try:
        subprocess.run([cc, "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-municode", "-static",
                        "-I", str(output), str(MIGRATION / "embedded-bootstrap.c"), "-lbcrypt", "-o", str(prefix)], check=True)
        bundle.append_bundle(prefix, archive, exe, version)
    finally:
        prefix.unlink(missing_ok=True)
    (output / "embedded-runtime.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    return exe


def validate_payload_directory(payload, native):
    payload = Path(payload)
    paths = {}
    for path in payload.rglob("*"):
        if path.is_symlink():
            raise ValueError("payload contains a symlink")
        if path.is_file():
            paths[path.relative_to(payload).as_posix()] = path
    if set(paths) != set(native["hashes"]) | {"SHA256SUMS.txt"}:
        raise ValueError("payload directory and native archive file set differ")
    for name, digest in native["hashes"].items():
        if bundle.digest_file(paths[name]) != digest:
            raise ValueError("payload directory differs from authenticated native archive")
    if bundle.parse_manifest(paths["SHA256SUMS.txt"].read_text(encoding="utf-8-sig")) != native["hashes"]:
        raise ValueError("payload directory manifest differs from native archive")
    if not set(LEGACY_FILES) <= paths.keys():
        raise ValueError("native package missing legacy transport support files")
    return paths


def native_archive_from_directory(output, payload, version):
    """Deterministic fallback for fixture/tooling; production can pass exact ZIP."""
    bundle.canonical_version(version)
    output, payload = Path(output), Path(payload)
    output.mkdir(parents=True, exist_ok=True)
    archive = output / f"grok-zh-{version}-windows-x86_64-msvc.zip"
    root = archive.stem
    with zipfile.ZipFile(archive, "x", compression=zipfile.ZIP_DEFLATED) as zipped:
        for path in sorted(payload.rglob("*")):
            if path.is_symlink():
                raise ValueError("native package contains a symlink")
            if path.is_file():
                info = zipfile.ZipInfo(f"{root}/{path.relative_to(payload).as_posix()}", date_time=(2026, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = 0o100644 << 16
                with path.open("rb") as source, zipped.open(info, "w", force_zip64=True) as destination:
                    shutil.copyfileobj(source, destination, bundle.CHUNK)
    bundle.validate_native_archive(archive, version)
    return archive


def build_compat_package(output, payload, version, *, release=True, archive=None):
    output, payload = Path(output), Path(payload)
    exe = output / "grok-zh.exe"
    # Always revalidate the bytes actually embedded in the launcher. Supplying
    # an external native path must not silently validate a different payload.
    native = bundle.validate_bundle(exe, version)
    if archive is not None and bundle.runtime_metadata(archive, version) != dict(
            schema=1, format=bundle.FORMAT, version=version, platform=bundle.TARGET,
            archive_size=native["archive_size"], archive_sha256=native["archive_sha256"]):
        raise ValueError("launcher and native archive differ")
    paths = validate_payload_directory(payload, native)
    metadata = dict(schema=1, format=bundle.FORMAT, version=version, platform=bundle.TARGET,
                    archive_size=native["archive_size"], archive_sha256=native["archive_sha256"])
    info = paths["BUILD-INFO.txt"].read_text(encoding="utf-8-sig").replace("\r\n", "\n")
    block = dict(native["protocol"], platform="x86_64-pc-windows-gnu", embedded_runtime=metadata)
    before = info.split("GROK-UPDATE-PROTOCOL-BEGIN", 1)[0].rstrip()
    before = re.sub(r"^Target:.*$", "Target: x86_64-pc-windows-gnu", before, flags=re.M)
    before = re.sub(r"^Grok Build.*$", "Grok Build Windows GNU embedded MSVC launcher" + ("" if release else " (experimental)"), before, flags=re.M)
    before = re.sub(r"^Profile:.*$", "Profile: GNU C launcher (-O2, static runtime); embedded native application is MSVC release-dist", before, flags=re.M)
    before += "\nRole: GNU compatibility transport; complete native MSVC payload is embedded for offline migration\n"
    name = f"grok-zh-{version}-windows-x86_64-gnu"
    package = output / name
    package.mkdir(exist_ok=False)
    for member in LEGACY_FILES:
        destination = package / member
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(exe if member == "grok-zh.exe" else paths[member], destination)
    (package / "BUILD-INFO.txt").write_text(before + "GROK-UPDATE-PROTOCOL-BEGIN\n" + json.dumps(block, indent=2) +
                                           "\nGROK-UPDATE-PROTOCOL-END\n", encoding="utf-8", newline="\n")
    (package / "SHA256SUMS.txt").write_text("".join(f"{bundle.digest_file(package / member)}  {member}\n"
        for member in sorted(LEGACY_FILES)), encoding="utf-8", newline="\n")
    packaged_sizes = [(package / member).stat().st_size for member in (*LEGACY_FILES, "SHA256SUMS.txt")]
    if max(packaged_sizes) > bundle.MAX_ARCHIVE_BYTES or sum(packaged_sizes) > bundle.MAX_EXPANDED_BYTES:
        raise ValueError("legacy transport exceeds historical updater extracted-size limits")
    result = output / f"{name}.zip"
    with zipfile.ZipFile(result, "x", compression=zipfile.ZIP_DEFLATED) as zipped:
        for path in sorted(package.rglob("*")):
            if path.is_file():
                zipped.write(path, f"{name}/{path.relative_to(package).as_posix()}")
    if result.stat().st_size > bundle.MAX_ARCHIVE_BYTES:
        result.unlink()
        raise ValueError("legacy transport ZIP exceeds historical updater 512 MiB limit")
    result.with_suffix(".zip.sha256").write_text(f"{bundle.digest_file(result)}  {result.name}\n", encoding="ascii")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--payload-package", type=Path, required=True)
    parser.add_argument("--payload-archive", type=Path)
    parser.add_argument("--cc", default="x86_64-w64-mingw32-gcc")
    args = parser.parse_args()
    archive = args.payload_archive or native_archive_from_directory(args.output, args.payload_package, args.version)
    native = bundle.validate_native_archive(archive, args.version)
    validate_payload_directory(args.payload_package, native)
    exe = build_launcher(args.output, args.version, archive, args.cc)
    transport = build_compat_package(args.output, args.payload_package, args.version, archive=archive)
    print(json.dumps(dict(launcher=str(exe), archive=str(transport), embedded_runtime=bundle.runtime_metadata(archive, args.version)), indent=2))


if __name__ == "__main__":
    main()
