"""Append the declarative update protocol before hashing BUILD-INFO.txt.

The block lives in an existing package member so legacy clients can validate
the exact same archive with their existing SHA256SUMS.txt parser.
"""

import argparse
import json
import re
from pathlib import Path

BEGIN = "GROK-UPDATE-PROTOCOL-BEGIN"
END = "GROK-UPDATE-PROTOCOL-END"
PLATFORMS = {
    "x86_64-pc-windows-gnu": ("grok-zh.exe", "Install-GrokZh.ps1"),
    "aarch64-pc-windows-msvc": ("grok-zh.exe", "Install-GrokZh.ps1"),
    "x86_64-apple-darwin": ("grok-zh", "Install-GrokZh.sh"),
    "aarch64-apple-darwin": ("grok-zh", "Install-GrokZh.sh"),
    "x86_64-unknown-linux-gnu": ("grok-zh", "Install-GrokZh.sh"),
    "aarch64-unknown-linux-gnu": ("grok-zh", "Install-GrokZh.sh"),
}


def append_protocol(package: Path, version: str, platform: str) -> None:
    if not re.fullmatch(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z.-]+)?", version):
        raise ValueError("invalid package version")
    executable, installer = PLATFORMS[platform]
    for name in (executable, installer, "BUILD-INFO.txt"):
        path = package / name
        if not path.is_file() or path.is_symlink():
            raise ValueError(f"missing regular package file: {name}")
    path = package / "BUILD-INFO.txt"
    text = path.read_text(encoding="utf-8-sig")
    if BEGIN in text or END in text:
        raise ValueError("package already contains an update protocol block")
    declared_version = re.search(r"^Version:[ \t]*(\S+)[ \t]*$", text, re.MULTILINE)
    if declared_version is None or declared_version.group(1) != version:
        raise ValueError("BUILD-INFO version does not match the package")
    protocol = dict(schema=1, version=version, platform=platform, mode="executable-only",
                    manifest="SHA256SUMS.txt", executable=executable, installer=installer)
    block = f"\n{BEGIN}\n{json.dumps(protocol, ensure_ascii=False, indent=2)}\n{END}\n"
    path.write_text(text.rstrip("\r\n") + "\n" + block, encoding="utf-8", newline="\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", required=True, type=Path)
    parser.add_argument("--version", required=True)
    parser.add_argument("--platform", required=True, choices=PLATFORMS)
    args = parser.parse_args()
    append_protocol(args.package, args.version, args.platform)
