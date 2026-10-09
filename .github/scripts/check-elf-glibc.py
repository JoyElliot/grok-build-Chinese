"""Fail when a Linux ELF requires a newer glibc symbol version than the published floor."""

import argparse
import os
import re
import subprocess
import sys


GLIBC_NAME = re.compile(r"^\s*0x[0-9a-f]+:\s+Name:\s+GLIBC_(\d+(?:\.\d+)+)\s+Flags:", re.M)
NEEDS_HEADER = "Version needs section '.gnu.version_r'"
VERSION = re.compile(r"^\d+\.\d+(?:\.\d+)?$")


def version_key(text):
    return tuple(int(part) for part in text.split("."))


def required_glibc_versions(version_info):
    """Return GLIBC_x.y names from readelf's verneed section only (GLIBC_PRIVATE is ignored)."""
    start = version_info.find(NEEDS_HEADER)
    if start < 0:
        raise ValueError("ELF has no .gnu.version_r section; cannot prove the glibc floor")
    section = version_info[start:]
    following = re.search(r"^\S.*section '", section[len(NEEDS_HEADER):], re.M)
    if following:
        section = section[:len(NEEDS_HEADER) + following.start()]
    versions = sorted(set(GLIBC_NAME.findall(section)), key=version_key)
    if not versions:
        raise ValueError("ELF references no GLIBC_ symbol version; refusing to guess the glibc floor")
    return versions


def symbols_newer_than(dyn_syms, limit):
    found = []
    for name, version in re.findall(r"\s(\S+)@GLIBC_(\d+(?:\.\d+)+)\b", dyn_syms):
        if version_key(version) > version_key(limit):
            found.append(f"{name}@GLIBC_{version}")
    return sorted(set(found))


def readelf(*arguments):
    env = dict(os.environ, LC_ALL="C")
    return subprocess.run(["readelf", *arguments], check=True, capture_output=True,
                          text=True, env=env).stdout


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True)
    parser.add_argument("--max", required=True, help="highest allowed glibc version, e.g. 2.35")
    parser.add_argument("--label", default="Linux")
    parser.add_argument("--summary", default=os.environ.get("GITHUB_STEP_SUMMARY", ""))
    args = parser.parse_args(argv)
    if not VERSION.match(args.max):
        parser.error(f"invalid --max version: {args.max}")

    versions = required_glibc_versions(readelf("--version-info", "--wide", args.binary))
    highest = versions[-1]
    passed = version_key(highest) <= version_key(args.max)
    offenders = [] if passed else symbols_newer_than(
        readelf("--dyn-syms", "--wide", args.binary), args.max)

    # Machine-readable line consumed by CI result collectors.
    print(f"GROK_ZH_MAX_GLIBC={highest}")
    print(f"{args.label}: highest required GLIBC_{highest}; allowed <= {args.max}; "
          f"all: {', '.join('GLIBC_' + v for v in versions)}")
    if args.summary:
        with open(args.summary, "a", encoding="utf-8") as summary:
            summary.write(f"### {args.label} glibc 基线\n\n")
            summary.write(f"- 二进制引用的最高 GLIBC 符号版本：`GLIBC_{highest}`\n")
            summary.write(f"- 允许上限：`GLIBC_{args.max}`；结果：{'通过' if passed else '失败'}\n")
            if offenders:
                summary.write(f"- 超出上限的符号：{', '.join(f'`{s}`' for s in offenders[:20])}\n")
            summary.write("\n")
    if not passed:
        listed = ", ".join(offenders[:20]) or "(readelf 未列出具体符号)"
        print(f"::error::{args.label} 二进制需要 GLIBC_{highest}，超过发布基线 {args.max}："
              f"{listed}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
