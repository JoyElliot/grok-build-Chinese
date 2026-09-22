"""Optional cache-key evidence; never publish raw logs or arbitrary environment."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess


MAX_LOG_BYTES = 32 * 1024 * 1024
MAX_KEYS = 4096
KEY = re.compile(r"[0-9a-f]{64}")
KEY_LINE = re.compile(
    r"\[[0-9T:.+Z-]+ DEBUG sccache::compiler::compiler\] "
    r"\[([A-Za-z_][A-Za-z0-9_]{0,127})\]: Hash key: ([0-9a-f]{64})"
)
# Only known public build settings, never a scan of the process environment.
CONTEXT_NAMES = (
    "CARGO_HOME", "CARGO_TARGET_DIR", "CARGO_INCREMENTAL", "RUSTUP_TOOLCHAIN",
    "CARGO_GROK_CACHE_INPUTS_SHA256", "GROK_VERSION", "TARGET", "RUSTC",
    "CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER", "PROTOC",
)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def file_sha256(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def disk_keys(root):
    """sccache disk layout is <first hex>/<second hex>/<full 64-hex key>."""
    keys = {}
    for path in root.glob("*/*/*"):
        key = path.name
        if KEY.fullmatch(key) and path.parent.name == key[1] and path.parent.parent.name == key[0]:
            if path.is_file() and not path.is_symlink():
                keys[key] = path.stat().st_size
                if len(keys) > MAX_KEYS:
                    raise ValueError("key limit")
    return dict(sorted(keys.items()))


def request_keys(raw):
    """Allowlist entire lines; paths, commands, errors and env never survive."""
    if len(raw) > MAX_LOG_BYTES:
        raise ValueError("log limit")
    result = []
    for line in raw.decode("utf-8", errors="replace").splitlines():
        match = KEY_LINE.fullmatch(line)
        if match:
            result.append({"crate": match[1], "key": match[2]})
            if len(result) > MAX_KEYS:
                raise ValueError("request limit")
    return result


def paths():
    run, attempt = os.environ["GITHUB_RUN_ID"], os.environ["GITHUB_RUN_ATTEMPT"]
    if not re.fullmatch(r"[0-9]+", run) or not re.fullmatch(r"[0-9]+", attempt):
        raise ValueError("invalid run identity")
    report = (Path(os.environ["CARGO_TARGET_DIR"]) / "cargo-timings" /
              f"sccache-key-evidence-{run}-{attempt}.json")
    private = Path(os.environ["RUNNER_TEMP"]) / f"grok-rust-cache-private-{run}-{attempt}.log"
    return report, private


def snapshot():
    if os.environ.get("GROK_SCCACHE_ACTIVE") != "true":
        return
    report, _ = paths()
    rustc = os.environ["RUSTC"]
    # Capture compiler output privately; failures must not print arbitrary stderr.
    def probe(*args):
        return subprocess.run([rustc, *args], check=True, capture_output=True,
                              text=True, timeout=15).stdout.strip()
    sysroot = Path(probe("--print", "sysroot"))
    libraries = sorted(sysroot.joinpath("bin").glob("*.dll"))
    # sccache hashes these DLL contents, not the launcher executable's mtime.
    dlls = {path.name: file_sha256(path) for path in libraries}
    data = {
        "schema": 1,
        "phase": "before_compile",
        "restored_keys": disk_keys(Path(os.environ["SCCACHE_DIR"])),
        "context_sha256": {name: sha256(os.environ[name].encode())
                           for name in CONTEXT_NAMES if name in os.environ},
        "cwd_sha256": sha256(str(Path.cwd()).encode()),
        "rustc_version_sha256": sha256(probe("-vV").encode()),
        "sysroot_dlls": dlls,
        "sysroot_dlls_sha256": sha256(json.dumps(dlls, sort_keys=True).encode()),
    }
    report.parent.mkdir(parents=True, exist_ok=True)
    report.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")


def summarize():
    if os.environ.get("GROK_SCCACHE_ACTIVE") != "true":
        return
    report, private = paths()
    snapshot_data = json.loads(report.read_text(encoding="utf-8"))
    data = {key: snapshot_data[key] for key in (
        "schema", "phase", "restored_keys", "context_sha256", "cwd_sha256",
        "rustc_version_sha256", "sysroot_dlls", "sysroot_dlls_sha256",
    ) if key in snapshot_data}
    with private.open("rb") as stream:
        requests = request_keys(stream.read(MAX_LOG_BYTES + 1))
    restored = data["restored_keys"]
    for request in requests:
        request["present_before_compile"] = request["key"] in restored
    data.update(phase="after_compile", requests=requests,
                matching_restored_requests=sum(r["present_before_compile"] for r in requests),
                final_keys=disk_keys(Path(os.environ["SCCACHE_DIR"])))
    report.write_text(json.dumps(data, indent=2) + "\n", encoding="utf-8")
    print(f"Cache key evidence: {len(requests)} requests; "
          f"{data['matching_restored_requests']} present before compilation.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=("snapshot", "summarize"))
    args = parser.parse_args()
    try:
        globals()[args.phase]()
    except Exception:
        # No exception details: they may contain compiler output or private paths.
        print("::warning::Optional cache-key evidence unavailable; compilation is unaffected.")


if __name__ == "__main__":
    main()
