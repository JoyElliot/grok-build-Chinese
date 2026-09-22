"""Optional, bounded compiler-result cache; never changes Cargo build options."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import time
import urllib.request
import zipfile


VERSION = "0.18.0"
ARCHIVE = f"sccache-v{VERSION}-x86_64-pc-windows-msvc.zip"
ARCHIVE_SHA256 = "8965c74d5e8a225244f741e18ad2f3f504f48228dc1bac948fc22761a348363d"
CACHE_BYTES = 256 * 1024 * 1024
ROOT = Path(__file__).resolve().parents[2]


def publish(path, values):
    with Path(path).open("a", encoding="utf-8") as stream:
        for key, value in values.items():
            if "\n" in str(value) or "\r" in str(value):
                raise ValueError(f"invalid multiline setting: {key}")
            stream.write(f"{key}={value}\n")


def resource_fingerprint(root, paths):
    """Hash non-Rust compile inputs, including directory additions/deletions.

    Rust sources/includes, externs and env! inputs are hashed by sccache itself.
    Extra resources read by procedural macros need this additional input.
    """
    digest = hashlib.sha256()
    for name in sorted(paths):
        path = Path(name)
        if path.suffix == ".rs" or path.parts[0] in (".github", "docs", "packaging"):
            continue
        data = (root / path).read_bytes()
        digest.update(name.encode("utf-8") + b"\0")
        digest.update(hashlib.sha256(data).digest())
    return digest.hexdigest()


def install(directory):
    directory.mkdir(parents=True, exist_ok=True)
    archive = directory / ARCHIVE
    url = f"https://github.com/mozilla/sccache/releases/download/v{VERSION}/{ARCHIVE}"
    if not archive.exists():
        for attempt in range(3):
            try:
                with urllib.request.urlopen(url, timeout=30) as response:
                    data = response.read()
                archive.write_bytes(data)
                break
            except OSError:
                if attempt == 2:
                    raise
                time.sleep(3)
    if hashlib.sha256(archive.read_bytes()).hexdigest() != ARCHIVE_SHA256:
        raise ValueError("sccache archive SHA-256 mismatch")
    # Extract only the pinned executable, not arbitrary archive paths.
    with zipfile.ZipFile(archive) as bundle:
        data = bundle.read(f"sccache-v{VERSION}-x86_64-pc-windows-msvc/sccache.exe")
    executable = directory / "sccache.exe"
    executable.write_bytes(data)
    actual = subprocess.check_output([str(executable), "--version"], text=True, timeout=10).strip()
    if actual != f"sccache {VERSION}":
        raise ValueError(f"unexpected sccache version: {actual}")
    return executable


def prepare():
    if os.environ.get("RUSTC_WRAPPER") or os.environ.get("RUSTC_WORKSPACE_WRAPPER"):
        raise ValueError("an existing compiler wrapper must not be replaced")
    if os.environ.get("CARGO_INCREMENTAL") != "0":
        raise ValueError("sccache requires the existing CARGO_INCREMENTAL=0 setting")
    temp = Path(os.environ["RUNNER_TEMP"])
    executable = install(temp / "grok-rust-cache-tool")
    wrapper = executable.with_name("grok-rustc-wrapper.exe")
    # A dedicated launcher avoids cc-rs implicitly using RUSTC_WRAPPER=sccache
    # for C/C++ too, and avoids filling this small cache with registry crates.
    # RUSTC_WORKSPACE_WRAPPER would change Cargo's artifact filename hashes.
    subprocess.run([
        os.environ["RUSTC"], str(ROOT / ".github/scripts/rust-cache-wrapper.rs"),
        "--edition=2021", "-Copt-level=1", f"-Clinker={os.environ['CC']}",
        "-o", str(wrapper),
    ], check=True, timeout=60)
    files = subprocess.check_output(["git", "ls-files", "-z"], cwd=ROOT).decode("utf-8").split("\0")
    fingerprint = resource_fingerprint(ROOT, [name for name in files if name])
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        port = listener.getsockname()[1]
    config = temp / "grok-rust-cache.toml"
    config.write_text("# CI uses only the bounded local disk backend.\n", encoding="utf-8")
    values = {
        "GROK_SCCACHE_EXE": str(executable),
        "GROK_RUST_CACHE_WRAPPER": str(wrapper),
        "SCCACHE_CONF": str(config),
        "SCCACHE_DIR": str(temp / "grok-rust-content-cache"),
        "SCCACHE_CACHE_SIZE": str(CACHE_BYTES),
        "SCCACHE_LOCAL_RW_MODE": os.environ.get("RUST_CACHE_MODE", "READ_ONLY"),
        "SCCACHE_IDLE_TIMEOUT": "0",
        "SCCACHE_SERVER_PORT": str(port),
        "SCCACHE_IGNORE_SERVER_IO_ERROR": "1",
        # v0.18.0 hashes CARGO_* in the Rust cache key. A cache archive key alone
        # would NOT invalidate restored results after a resource file changes.
        "CARGO_GROK_CACHE_INPUTS_SHA256": fingerprint,
    }
    if values["SCCACHE_LOCAL_RW_MODE"] not in ("READ_ONLY", "READ_WRITE"):
        raise ValueError("invalid cache access mode")
    publish(os.environ["GITHUB_ENV"], values)
    publish(os.environ["GITHUB_OUTPUT"], {"prepared": "true"})
    print(f"Prepared sccache {VERSION}; disk limit {CACHE_BYTES} bytes; resource hash {fingerprint}")


def activate():
    executable = os.environ["GROK_SCCACHE_EXE"]
    # Start only after cache restore. Failed setup leaves the normal rustc path.
    subprocess.run([executable, "--start-server"], check=True, timeout=20)
    subprocess.run([executable, "--zero-stats"], check=True, timeout=10)
    publish(os.environ["GITHUB_ENV"], {
        "RUSTC_WRAPPER": os.environ["GROK_RUST_CACHE_WRAPPER"],
        "GROK_SCCACHE_ACTIVE": "true", "GROK_SCCACHE_SAVE_READY": "false",
    })
    publish(os.environ["GITHUB_OUTPUT"], {"enabled": "true"})


def finish():
    if os.environ.get("GROK_SCCACHE_ACTIVE") != "true":
        print("Compiler-result cache inactive; Cargo used the normal compiler.")
        return
    executable = os.environ["GROK_SCCACHE_EXE"]
    timings = Path(os.environ["CARGO_TARGET_DIR"]) / "cargo-timings"
    timings.mkdir(parents=True, exist_ok=True)
    try:
        data = subprocess.check_output(
            [executable, "--show-stats", "--stats-format=json"], text=True, timeout=15,
        )
        stats = json.loads(data)
        (timings / "sccache-stats.json").write_text(json.dumps(stats, indent=2), encoding="utf-8")
        result = subprocess.check_output([executable, "--show-stats"], text=True, timeout=15)
        print(result)
        (timings / "sccache-stats.txt").write_text(result, encoding="utf-8")
        if os.environ.get("GITHUB_STEP_SUMMARY"):
            with Path(os.environ["GITHUB_STEP_SUMMARY"]).open("a", encoding="utf-8") as stream:
                stream.write("\n### Windows compiler-result cache\n\n```text\n" + result + "\n```\n")
    finally:
        # Drain cache writes before the existing immutable-cache save helper.
        subprocess.run([executable, "--stop-server"], timeout=20, check=True)
    # A failed stats request or failed stop skips saving, without failing Cargo.
    publish(os.environ["GITHUB_ENV"], {"GROK_SCCACHE_SAVE_READY": "true"})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=("prepare", "activate", "finish"))
    args = parser.parse_args()
    try:
        globals()[args.phase]()
    except Exception as error:
        # This optional optimization must not add a build gate. Cargo, package
        # validation and test failure propagation remain in the calling workflow.
        print(f"::warning::Optional Rust compiler cache {args.phase} unavailable: {error}")


if __name__ == "__main__":
    main()
