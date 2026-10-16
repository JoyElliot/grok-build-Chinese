"""Build the opt-in GNU migration launcher and its small immutable helper.

This produces local/CI artifacts only. It never creates a GitHub Release or
changes the normal six-platform release matrix. The chosen helper tag must be
published immutably before distributing a launcher that pins it.
"""

import argparse
import hashlib
import json
import re
import subprocess
import shutil
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
MIGRATION = ROOT / "packaging/windows/migration"
ASSET = "grok-zh-migration-windows-x64.zip"
MIGRATION_ID = "windows-x64-gnu-to-msvc-v1"
LEGACY_FILES = (
    "grok-zh.exe", "agent-zh.cmd", "rg.exe", "一键安装.cmd", "[可选]替换原始启动方式.cmd",
    "Install-GrokZh.ps1", "INSTALL-WINDOWS.md", "LICENSE-grok-build.txt", "BUILD-INFO.txt",
    "licenses/ripgrep/COPYING", "licenses/ripgrep/LICENSE-MIT", "licenses/ripgrep/UNLICENSE",
    "licenses/project/THIRD-PARTY-NOTICES", "licenses/project/THIRD_PARTY_NOTICES.md", "licenses/project/NOTICE",
)


def script_bytes(path: Path) -> bytes:
    # Windows PowerShell 5.1 requires the BOM for non-ASCII source code.
    return b"\xef\xbb\xbf" + path.read_text(encoding="utf-8-sig").replace("\r\n", "\n").encode("utf-8")


def build_helper(output: Path, tag: str) -> dict:
    if not re.fullmatch(r"migration-windows-x64-v[1-9][0-9]*", tag):
        raise ValueError("invalid immutable migration release tag")
    metadata = dict(schema=1, id=MIGRATION_ID, **{"from": "x86_64-pc-windows-gnu"},
                    to="x86_64-pc-windows-msvc", entry="Invoke-Migration.ps1")
    files = {
        "MIGRATION.json": (json.dumps(metadata, indent=2) + "\n").encode(),
        "Invoke-Migration.ps1": script_bytes(MIGRATION / "Invoke-Migration.ps1"),
    }
    files["SHA256SUMS.txt"] = "".join(
        f"{hashlib.sha256(data).hexdigest()}  {name}\n" for name, data in sorted(files.items())
    ).encode()
    output.mkdir(parents=True, exist_ok=True)
    archive = output / ASSET
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as z:
        for name, data in sorted(files.items()):
            info = zipfile.ZipInfo(name, date_time=(2026, 1, 1, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = 0o100644 << 16
            z.writestr(info, data)
    data = archive.read_bytes()
    pin = dict(id=MIGRATION_ID, tag=tag, asset=ASSET, size=len(data), sha256=hashlib.sha256(data).hexdigest())
    (output / "migration-pin.json").write_text(json.dumps(pin, indent=2) + "\n", encoding="utf-8")
    return pin


def c_array(name: str, data: bytes) -> str:
    rows = [", ".join(str(n) for n in data[i:i + 32]) for i in range(0, len(data), 32)]
    return f"static const unsigned char {name}[] = {{\n" + ",\n".join(rows) + "\n};\n"


def build_launcher(output: Path, version: str, pin: dict, cc: str) -> Path:
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)", version):
        raise ValueError("the legacy launcher requires a canonical stable version")
    header = f"#define BOOTSTRAP_VERSION {json.dumps(version)}\n"
    header += f"#define MIGRATION_PIN {json.dumps(json.dumps(pin, separators=(',', ':')))}\n"
    header += c_array("bootstrap_script", script_bytes(MIGRATION / "Invoke-Bootstrap.ps1"))
    header += c_array("online_script", script_bytes(ROOT / "packaging/windows/Install-GrokZhOnline.ps1"))
    (output / "bootstrap_payload.h").write_text(header, encoding="ascii")
    exe = output / "grok-zh.exe"
    subprocess.run([cc, "-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-municode", "-static",
                    "-I", str(output), str(MIGRATION / "bootstrap.c"), "-lbcrypt", "-o", str(exe)], check=True)
    return exe


def build_compat_package(output: Path, payload: Path, version: str) -> Path:
    """Keep the old GNU filename/protocol truthful: its entry is the GNU shim.

    Reuse only authenticated package support files, never rename an MSVC EXE
    into a supposedly GNU build. MSVC remains a distinct archive and target.
    """
    manifest = (payload / "SHA256SUMS.txt").read_text(encoding="utf-8-sig")
    files = {}
    for line in manifest.splitlines():
        match = re.fullmatch(r"([0-9a-fA-F]{64})  (.+)", line)
        if not match:
            raise ValueError("invalid payload manifest")
        digest, name = match.groups()
        if (name.startswith(("/", "\\")) or "\\" in name or ":" in name or
                any(part in ("", ".", "..") for part in name.split("/")) or name.lower() in files or
                name.lower() == "sha256sums.txt"):
            raise ValueError("unsafe/duplicate payload member")
        path = payload / name
        if not path.is_file() or any(p.is_symlink() for p in [path, *path.parents]):
            raise ValueError("payload member is not a regular file")
        data = path.read_bytes()
        if hashlib.sha256(data).hexdigest() != digest.lower():
            raise ValueError("payload hash mismatch")
        files[name.lower()] = (name, data)
    # Freeze the legacy entry's exact file set even if the MSVC package grows.
    # Additional runtime resources are delivered by the separate full payload.
    files = {name.lower(): (name, files[name.lower()][1]) for name in LEGACY_FILES}
    info = files["build-info.txt"][1].decode("utf-8-sig").replace("\r\n", "\n")
    if not re.search(r"^Target:[ \t]*x86_64-pc-windows-msvc[ \t]*$", info, re.M):
        raise ValueError("the payload must be a real x64 MSVC package")
    block = json.loads(info.split("GROK-UPDATE-PROTOCOL-BEGIN\n", 1)[1].split("\nGROK-UPDATE-PROTOCOL-END", 1)[0])
    if (block != dict(schema=1, version=version, platform="x86_64-pc-windows-msvc", mode="executable-only",
                      manifest="SHA256SUMS.txt", executable="grok-zh.exe", installer="Install-GrokZh.ps1")):
        raise ValueError("payload protocol mismatch")
    name = f"grok-zh-{version}-windows-x86_64-gnu"
    package = output / name
    package.mkdir(exist_ok=False)
    for _, (member, data) in files.items():
        path = package / member
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    shutil.copyfile(output / "grok-zh.exe", package / "grok-zh.exe")
    block["platform"] = "x86_64-pc-windows-gnu"
    before = info.split("GROK-UPDATE-PROTOCOL-BEGIN", 1)[0].rstrip()
    before = re.sub(r"^Target:.*$", "Target: x86_64-pc-windows-gnu", before, flags=re.M)
    before = re.sub(r"^Grok Build.*$", "Grok Build Windows GNU migration launcher (experimental)", before, flags=re.M)
    before = re.sub(r"^Profile:.*$", "Profile: GNU C launcher (-O2, static runtime); application remains MSVC release-dist", before, flags=re.M)
    before = re.sub(r"^Executable smoke-tested by CI:.*$", "Executable validation: see this run's migration experiment result", before, flags=re.M)
    before += "\nRole: GNU compatibility launcher; MSVC payload is downloaded once on first normal launch\n"
    (package / "BUILD-INFO.txt").write_text(before + "GROK-UPDATE-PROTOCOL-BEGIN\n" + json.dumps(block, indent=2) +
                                           "\nGROK-UPDATE-PROTOCOL-END\n", encoding="utf-8", newline="\n")
    hashes = "".join(f"{hashlib.sha256((package / member).read_bytes()).hexdigest()}  {member}\n"
                     for member, _ in sorted(files.values()))
    (package / "SHA256SUMS.txt").write_text(hashes, encoding="utf-8", newline="\n")
    archive = output / f"{name}.zip"
    with zipfile.ZipFile(archive, "w", compression=zipfile.ZIP_DEFLATED) as z:
        for path in sorted(package.rglob("*")):
            if path.is_file():
                z.write(path, f"{name}/{path.relative_to(package).as_posix()}")
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_suffix(".zip.sha256").write_text(f"{digest}  {archive.name}\n", encoding="ascii")
    return archive


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--migration-tag", required=True)
    parser.add_argument("--cc", default="x86_64-w64-mingw32-gcc")
    parser.add_argument("--payload-package", type=Path, help="Optional verified x64 MSVC package directory; creates the old GNU-compatible ZIP")
    args = parser.parse_args()
    pin = build_helper(args.output, args.migration_tag)
    exe = build_launcher(args.output, args.version, pin, args.cc)
    if args.payload_package:
        build_compat_package(args.output, args.payload_package, args.version)
    print(json.dumps({"launcher": str(exe), "size": exe.stat().st_size, "migration": pin}, indent=2))


if __name__ == "__main__":
    main()
