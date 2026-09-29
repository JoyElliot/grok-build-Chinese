"""Bind an experimental cross-built MSVC EXE to its native acceptance job."""

import argparse
import hashlib
import json
import os
import shutil
from pathlib import Path


def identity():
    return {
        "schema": 1,
        "commit": os.environ["GITHUB_SHA"],
        "run_id": os.environ["GITHUB_RUN_ID"],
        "run_attempt": os.environ["GITHUB_RUN_ATTEMPT"],
        "version": os.environ["GROK_VERSION"],
        "target": "x86_64-pc-windows-msvc",
        "build_host": "aarch64-pc-windows-msvc",
        "profile": "release-dist",
        "features": "release-dist",
    }


def describe(executable):
    data = executable.read_bytes()
    # Architecture is checked again by the native package verifier.
    return dict(identity(), size=len(data), sha256=hashlib.sha256(data).hexdigest())


def write_input(executable, directory):
    directory.mkdir(parents=True, exist_ok=False)
    staged = directory / "grok-zh.exe"
    shutil.copyfile(executable, staged)
    (directory / "build-input.json").write_text(
        json.dumps(describe(staged), indent=2) + "\n", encoding="utf-8"
    )


def verify_input(directory, destination):
    if {p.name for p in directory.iterdir()} != {"grok-zh.exe", "build-input.json"}:
        raise ValueError("unexpected build input members")
    executable = directory / "grok-zh.exe"
    manifest = json.loads((directory / "build-input.json").read_text(encoding="utf-8"))
    if manifest != describe(executable):
        raise ValueError("build input identity, size or SHA-256 mismatch")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(executable, destination)
    if describe(destination) != manifest:
        raise ValueError("staged MSVC executable differs from verified build input")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("write", "verify"))
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--directory", type=Path, required=True)
    args = parser.parse_args()
    if args.mode == "write":
        write_input(args.executable, args.directory)
    else:
        verify_input(args.directory, args.executable)


if __name__ == "__main__":
    main()
