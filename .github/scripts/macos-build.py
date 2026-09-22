"""Select the macOS build profile and record Cargo build resource samples."""

import argparse
import csv
import datetime
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import time


def build_config(release_build, variant="current"):
    if variant not in ("current", "thin-lto"):
        raise ValueError("unknown macOS build variant")
    if variant == "thin-lto":
        if release_build:
            raise ValueError("unverified macOS profile is restricted to CI experiments")
        return {
            "profile": "release-dist",
            "cache_key": "trial-release-dist-ltothin-debug0-cgu1-opt3-v1",
            "description": "release-dist (Thin LTO, debug=0, codegen-units=1, opt-level=3; CI experiment)",
            "overrides": ["profile.release-dist.debug=0"],
        }
    build_kind = "Release" if release_build else "CI preview"
    return {
        "profile": "release",
        # Keep the verified preview key so tags can restore the same profile cache.
        "cache_key": "preview-release-lto0-debug0-cgu16-shellopt1-shellcgu16-v1",
        "description": (
            "release with release-dist features (cross-crate LTO disabled, debug=0, "
            f"codegen-units=16, shell opt-level=1/codegen-units=16; {build_kind})"
        ),
        "overrides": [
            "profile.release.lto=false",
            "profile.release.debug=0",
            "profile.release.codegen-units=16",
            "profile.release.package.xai-grok-shell.opt-level=1",
            "profile.release.package.xai-grok-shell.codegen-units=16",
        ],
    }


def cache_writable(release_build, event, ref):
    if release_build:
        return False
    if ref == "refs/heads/zh-dev" and event in ("push", "workflow_dispatch"):
        return True
    return event == "workflow_dispatch" and ref.startswith("refs/heads/sync/upstream-")


def cargo_command(release_build, target, jobs, variant="current"):
    if target != "aarch64-apple-darwin" or jobs < 1:
        raise ValueError("expected the macOS ARM64 target and a positive job count")
    config = build_config(release_build, variant)
    command = [
        "cargo", "build", "--frozen", "-j", str(jobs), "--target", target,
        "-p", "xai-grok-pager-bin", "--profile", config["profile"],
        "--features", "release-dist", "--timings",
    ]
    for override in config["overrides"]:
        command.extend(("--config", override))
    return command


def process_tree_sample(text, root_pid):
    processes = {}
    for line in text.splitlines():
        fields = line.split(None, 3)
        if len(fields) != 4:
            continue
        try:
            pid, parent_pid, rss_kib = map(int, fields[:3])
        except ValueError:
            continue
        processes[pid] = (parent_pid, rss_kib, fields[3])
    if root_pid not in processes:
        return {"process_count": None, "rustc_count": None, "process_tree_rss_mib": None}
    descendants = {root_pid}
    pending = [root_pid]
    while pending:
        parent = pending.pop()
        for pid, (parent_pid, _, _) in processes.items():
            if parent_pid == parent and pid not in descendants:
                descendants.add(pid)
                pending.append(pid)
    selected = [processes[pid] for pid in descendants if pid in processes]
    return {
        "process_count": len(selected),
        "rustc_count": sum(Path(name).name == "rustc" for _, _, name in selected),
        "process_tree_rss_mib": round(sum(rss for _, rss, _ in selected) / 1024, 2),
    }


def swap_used_mib(text):
    match = re.search(r"\bused\s*=\s*([\d.]+)([KMGT])\b", text)
    if not match:
        raise ValueError("unrecognized vm.swapusage output")
    return round(float(match[1]) * {"K": 1 / 1024, "M": 1, "G": 1024, "T": 1024**2}[match[2]], 2)


def probe(command, warnings):
    try:
        return subprocess.run(
            command, check=True, capture_output=True, text=True, timeout=5,
        ).stdout.strip()
    except (OSError, subprocess.SubprocessError) as error:
        warnings.add(f"{' '.join(command)}: {type(error).__name__}")
        return None


def resource_sample(root_pid, warnings):
    sample = {"process_count": None, "rustc_count": None, "process_tree_rss_mib": None}
    process_text = probe(["ps", "-axo", "pid=,ppid=,rss=,comm="], warnings)
    if process_text is not None:
        sample.update(process_tree_sample(process_text, root_pid))
        if sample["process_count"] is None:
            warnings.add("Cargo process was not present in the process snapshot")
    sample["system_swap_used_mib"] = None
    swap_text = probe(["sysctl", "vm.swapusage"], warnings)
    if swap_text is not None:
        try:
            sample["system_swap_used_mib"] = swap_used_mib(swap_text)
        except ValueError as error:
            warnings.add(str(error))
    return sample


def stop_build(process):
    # A separate POSIX session lets cancellation reach rustc/linker descendants
    # even when Cargo has already exited. This script only signals its own group.
    if os.name == "posix":
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            pass
        finally:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
    elif process.poll() is None:
        process.terminate()
        try:
            process.wait(timeout=10)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()


def handle_termination(signum, _frame):
    raise SystemExit(128 + signum)


def run_build(command, report_dir, interval, sampler=resource_sample):
    report_dir.mkdir(parents=True, exist_ok=True)
    warnings = set()
    started = time.monotonic()
    started_at = datetime.datetime.now(datetime.timezone.utc).isoformat()
    samples = []
    columns = [
        "elapsed_seconds", "process_count", "rustc_count", "process_tree_rss_mib",
        "system_swap_used_mib",
    ]
    print("Running: " + " ".join(command), flush=True)
    with (report_dir / "samples.csv").open("w", encoding="utf-8", newline="") as output:
        writer = csv.DictWriter(output, fieldnames=columns)
        writer.writeheader()
        with subprocess.Popen(command, start_new_session=os.name == "posix") as process:
            previous_sigterm = signal.signal(signal.SIGTERM, handle_termination)
            try:
                while True:
                    sampled_at = time.monotonic()
                    sample = sampler(process.pid, warnings)
                    sample["elapsed_seconds"] = round(sampled_at - started, 2)
                    samples.append(sample)
                    writer.writerow(sample)
                    output.flush()
                    try:
                        remaining = max(0, sampled_at + interval - time.monotonic())
                        exit_code = process.wait(timeout=remaining)
                        break
                    except subprocess.TimeoutExpired:
                        pass
            except BaseException:
                stop_build(process)
                raise
            finally:
                signal.signal(signal.SIGTERM, previous_sigterm)
            if exit_code != 0:
                stop_build(process)
    duration = round(time.monotonic() - started, 2)

    def maximum(field):
        values = [sample[field] for sample in samples if sample[field] is not None]
        return max(values) if values else None

    report = {
        "started_at": started_at,
        "duration_seconds": duration,
        "exit_code": exit_code,
        "command": command,
        "sample_interval_seconds": interval,
        "sample_count": len(samples),
        "max_sampled_process_tree_rss_mib": maximum("process_tree_rss_mib"),
        "max_sampled_system_swap_used_mib": maximum("system_swap_used_mib"),
        "max_sampled_rustc_count": maximum("rustc_count"),
        "sampling_warnings": sorted(warnings),
        "measurement_notes": (
            "RSS is the sampled sum for Cargo and its descendants, not a unique-memory "
            "or exact-peak measurement. Swap is runner-wide usage. Missing probes remain null."
        ),
    }
    (report_dir / "summary.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2), flush=True)
    return exit_code, report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("configure", "build"))
    parser.add_argument("--release-build", choices=("true", "false"), required=True)
    parser.add_argument("--variant", choices=("current", "thin-lto"),
                        default=os.environ.get("MACOS_BUILD_VARIANT", "current"))
    parser.add_argument("--sample-seconds", type=float, default=10)
    args = parser.parse_args()
    release_build = args.release_build == "true"
    config = build_config(release_build, args.variant)
    if args.mode == "configure":
        values = {
            "MACOS_BUILD_VARIANT": args.variant,
            "MACOS_CARGO_PROFILE": config["profile"],
            "MACOS_PROFILE_CACHE_KEY": config["cache_key"],
            "MACOS_PROFILE_DESCRIPTION": config["description"],
            "MACOS_CACHE_WRITABLE": str(args.variant == "current" and cache_writable(
                release_build, os.environ["GITHUB_EVENT_NAME"], os.environ["GITHUB_REF"],
            )).lower(),
            "MACOS_BUILD_REPORT_DIR": str(Path(os.environ["RUNNER_TEMP"]) / (
                f"grok-zh-macos-build-{os.environ['GITHUB_RUN_ID']}-{os.environ['GITHUB_RUN_ATTEMPT']}"
            )),
        }
        for name, value in values.items():
            if "\n" in value or "\r" in value:
                raise ValueError("multiline GitHub environment value")
            print(f"{name}={value}")
        return 0

    if not 1 <= args.sample_seconds <= 60:
        parser.error("--sample-seconds must be between 1 and 60")
    if os.environ["MACOS_CARGO_PROFILE"] != config["profile"]:
        raise ValueError("configured cache/package profile does not match the build mode")
    report_dir = Path(os.environ["MACOS_BUILD_REPORT_DIR"])
    command = cargo_command(release_build, os.environ["TARGET"], int(os.environ["CARGO_BUILD_JOBS"]), args.variant)
    hardware_warnings = set()
    hardware = {
        "logical_cpu_count": probe(["sysctl", "-n", "hw.logicalcpu"], hardware_warnings),
        "memory_bytes": probe(["sysctl", "-n", "hw.memsize"], hardware_warnings),
        "warnings": sorted(hardware_warnings),
    }
    report_dir.mkdir(parents=True, exist_ok=True)
    (report_dir / "runner.json").write_text(json.dumps(hardware, indent=2) + "\n", encoding="utf-8")
    exit_code, report = run_build(command, report_dir, args.sample_seconds)
    summary_path = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary_path:
        with Path(summary_path).open("a", encoding="utf-8") as output:
            output.write(
                "\n### macOS Cargo build measurements\n\n"
                f"- Profile: `{config['description']}`\n"
                f"- Cargo build duration: {report['duration_seconds'] / 60:.2f} minutes\n"
                f"- Cargo exit code: {exit_code}\n"
                f"- Resource samples: {report['sample_count']}\n"
                f"- Maximum sampled rustc count: {report['max_sampled_rustc_count']}\n"
                f"- Maximum sampled process-tree RSS: {report['max_sampled_process_tree_rss_mib']} MiB\n"
                f"- Maximum sampled runner swap: {report['max_sampled_system_swap_used_mib']} MiB\n"
                "- Sampled RSS sums Cargo and descendants; it is not exact peak or unique memory.\n"
            )
    return exit_code if exit_code >= 0 else 128 - exit_code


if __name__ == "__main__":
    sys.exit(main())
