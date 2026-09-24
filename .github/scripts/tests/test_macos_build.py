import contextlib
import csv
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import signal
import sys
import tempfile
import time
import unittest
from unittest import mock


sys.dont_write_bytecode = True
SCRIPT = Path(__file__).resolve().parents[1] / "macos-build.py"
spec = importlib.util.spec_from_file_location("macos_build", SCRIPT)
macos_build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(macos_build)
TARGET = "aarch64-apple-darwin"


class MacosBuildTests(unittest.TestCase):
    def test_both_native_macos_targets_use_the_release_command(self):
        for target in ("aarch64-apple-darwin", "x86_64-apple-darwin"):
            with self.subTest(target=target):
                command = macos_build.cargo_command(True, target, 3)
                self.assertEqual(command[command.index("--target") + 1], target)
        with self.assertRaisesRegex(ValueError, "supported macOS target"):
            macos_build.cargo_command(True, "aarch64-unknown-linux-gnu", 3)

    def test_thin_lto_trial_is_explicit_isolated_and_not_a_release_profile_switch(self):
        command = macos_build.cargo_command(False, TARGET, 3, "thin-lto")
        self.assertEqual(command[command.index("--profile") + 1], "release-dist")
        self.assertEqual(command[command.index("--features") + 1], "release-dist")
        self.assertEqual(command[-2:], ["--config", "profile.release-dist.debug=0"])
        self.assertNotEqual(macos_build.build_config(False)["cache_key"],
                            macos_build.build_config(False, "thin-lto")["cache_key"])
        with self.assertRaisesRegex(ValueError, "restricted"):
            macos_build.build_config(True, "thin-lto")
        with self.assertRaisesRegex(ValueError, "unknown"):
            macos_build.build_config(False, "invalid")

    def test_formal_release_reuses_the_verified_preview_command_and_cache(self):
        self.assertEqual(
            macos_build.cargo_command(True, TARGET, 3),
            macos_build.cargo_command(False, TARGET, 3),
        )
        self.assertEqual(
            macos_build.build_config(True)["cache_key"],
            macos_build.build_config(False)["cache_key"],
        )
        self.assertIn("; Release)", macos_build.build_config(True)["description"])
        self.assertIn("; CI preview)", macos_build.build_config(False)["description"])

    def test_default_profile_keeps_current_optimization_settings(self):
        command = macos_build.cargo_command(False, TARGET, 3)
        self.assertEqual(command[command.index("--profile") + 1], "release")
        self.assertEqual(command[command.index("--features") + 1], "release-dist")
        overrides = [command[i + 1] for i, value in enumerate(command) if value == "--config"]
        self.assertEqual(set(overrides), {
            "profile.release.lto=false",
            "profile.release.debug=0",
            "profile.release.codegen-units=16",
            "profile.release.package.xai-grok-shell.opt-level=1",
            "profile.release.package.xai-grok-shell.codegen-units=16",
        })
        self.assertEqual(
            macos_build.build_config(False)["cache_key"],
            "preview-release-lto0-debug0-cgu16-shellopt1-shellcgu16-v1",
        )
        self.assertIn("--frozen", command)
        self.assertIn("--timings", command)

    def test_only_trusted_preview_events_can_save_cache(self):
        cases = [
            (False, "push", "refs/heads/zh-dev", True),
            (False, "workflow_dispatch", "refs/heads/zh-dev", True),
            (False, "workflow_dispatch", "refs/heads/sync/upstream-1.0.24", True),
            (False, "push", "refs/heads/sync/upstream-1.0.24", False),
            (False, "workflow_dispatch", "refs/heads/unrelated", False),
            (False, "pull_request", "refs/pull/1/merge", False),
            (False, "pull_request_target", "refs/heads/zh-dev", False),
            (False, "workflow_run", "refs/heads/zh-dev", False),
            (True, "workflow_dispatch", "refs/heads/zh-dev", False),
            (True, "workflow_dispatch", "refs/heads/sync/upstream-1.0.24", False),
            (True, "push", "refs/tags/release-v1.0.24", False),
        ]
        for release, event, ref, expected in cases:
            with self.subTest(release=release, event=event, ref=ref):
                self.assertEqual(macos_build.cache_writable(release, event, ref), expected)

    def test_configure_emits_matching_cache_package_and_build_profiles(self):
        with tempfile.TemporaryDirectory() as folder:
            env = os.environ | {
                "MACOS_BUILD_VARIANT": "current",
                "GITHUB_EVENT_NAME": "workflow_dispatch",
                "GITHUB_REF": "refs/heads/sync/upstream-1.0.24",
                "GITHUB_RUN_ID": "123", "GITHUB_RUN_ATTEMPT": "2", "RUNNER_TEMP": folder,
            }
            for mode, profile, writable in (("true", "release", "false"), ("false", "release", "true")):
                with self.subTest(mode=mode):
                    result = subprocess.run(
                        [sys.executable, str(SCRIPT), "configure", "--release-build", mode],
                        env=env, check=True, capture_output=True, text=True,
                    )
                    values = dict(line.split("=", 1) for line in result.stdout.splitlines())
                    self.assertEqual(values["MACOS_CARGO_PROFILE"], profile)
                    self.assertEqual(values["MACOS_CACHE_WRITABLE"], writable)
                    self.assertEqual(
                        Path(values["MACOS_BUILD_REPORT_DIR"]),
                        Path(folder) / "grok-zh-macos-build-123-2",
                    )
                    self.assertFalse(Path(values["MACOS_BUILD_REPORT_DIR"]).exists())

    def test_configure_trial_from_environment_is_read_only_and_rejects_release(self):
        with tempfile.TemporaryDirectory() as folder:
            env = os.environ | {
                "MACOS_BUILD_VARIANT": "thin-lto",
                "GITHUB_EVENT_NAME": "workflow_dispatch",
                "GITHUB_REF": "refs/heads/zh-dev",
                "GITHUB_RUN_ID": "123", "GITHUB_RUN_ATTEMPT": "2", "RUNNER_TEMP": folder,
            }
            command = [sys.executable, str(SCRIPT), "configure", "--release-build"]
            result = subprocess.run(command + ["false"], env=env, check=True,
                                    capture_output=True, text=True)
            values = dict(line.split("=", 1) for line in result.stdout.splitlines())
            self.assertEqual(values["MACOS_BUILD_VARIANT"], "thin-lto")
            self.assertEqual(values["MACOS_CARGO_PROFILE"], "release-dist")
            self.assertEqual(values["MACOS_CACHE_WRITABLE"], "false")
            self.assertEqual(values["MACOS_PROFILE_CACHE_KEY"],
                             macos_build.build_config(False, "thin-lto")["cache_key"])
            result = subprocess.run(command + ["true"], env=env, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("restricted to CI experiments", result.stderr)

    def test_process_samples_include_descendants_and_exclude_unrelated_compilers(self):
        processes = """
            100 1 1024 /usr/bin/cargo
            110 100 2048 /toolchain/bin/rustc
            120 110 4096 /usr/bin/clang
            130 1 999999 /other/rustc
            malformed row
        """
        sample = macos_build.process_tree_sample(processes, 100)
        self.assertEqual(sample["process_count"], 3)
        self.assertEqual(sample["rustc_count"], 1)
        self.assertEqual(sample["process_tree_rss_mib"], 7)

    def test_swap_units_and_missing_probes_are_not_reported_as_zero(self):
        for value, expected in (("1024.00K", 1), ("12.50M", 12.5), ("1.25G", 1280), ("1.00T", 1024**2)):
            with self.subTest(value=value):
                self.assertEqual(macos_build.swap_used_mib(f"total = 2G used = {value} free = 0M"), expected)
        with self.assertRaises(ValueError):
            macos_build.swap_used_mib("unavailable")
        with mock.patch.object(macos_build, "probe", return_value=None):
            sample = macos_build.resource_sample(123, set())
        self.assertTrue(all(value is None for value in sample.values()))

    def test_missing_root_and_empty_process_output_remain_unknown(self):
        for text in ("", "unrecognized output", "999 1 1024 /other/rustc"):
            with self.subTest(text=text):
                sample = macos_build.process_tree_sample(text, 123)
                self.assertTrue(all(value is None for value in sample.values()))

    def test_build_exit_code_and_diagnostics_survive_success_and_failure(self):
        def sample(_pid, _warnings):
            return {
                "process_count": 1, "rustc_count": 0,
                "process_tree_rss_mib": 32, "system_swap_used_mib": None,
            }

        for expected_code in (0, 7):
            with self.subTest(exit_code=expected_code), tempfile.TemporaryDirectory() as folder:
                report_dir = Path(folder) / "reports"
                with contextlib.redirect_stdout(io.StringIO()):
                    code, report = macos_build.run_build(
                        [sys.executable, "-c", f"import sys; sys.exit({expected_code})"],
                        report_dir, 0.05, sampler=sample,
                    )
                self.assertEqual(code, expected_code)
                persisted = json.loads((report_dir / "summary.json").read_text(encoding="utf-8"))
                self.assertEqual(persisted, report)
                self.assertEqual(report["exit_code"], expected_code)
                self.assertEqual(report["max_sampled_process_tree_rss_mib"], 32)
                self.assertIsNone(report["max_sampled_system_swap_used_mib"])
                with (report_dir / "samples.csv").open(encoding="utf-8", newline="") as data:
                    self.assertEqual(len(list(csv.DictReader(data))), report["sample_count"])

    @unittest.skipUnless(os.name == "posix", "macOS/POSIX process-group cancellation")
    def test_sigterm_stops_cargo_and_its_descendant(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            child_pid_path = root / "child-pid"
            # The descendant ignores SIGTERM to verify the final group cleanup.
            child_code = (
                "import os,pathlib,signal,time; "
                "signal.signal(signal.SIGTERM, signal.SIG_IGN); "
                f"pathlib.Path({str(child_pid_path)!r}).write_text(str(os.getpid())); "
                "time.sleep(30)"
            )
            cargo_code = (
                "import subprocess,sys,time; "
                f"subprocess.Popen([sys.executable, '-c', {child_code!r}]); "
                "time.sleep(30)"
            )
            pids = []

            def cancel_when_ready(pid, _warnings):
                pids.append(pid)
                deadline = time.monotonic() + 5
                while time.monotonic() < deadline:
                    if child_pid_path.exists() and child_pid_path.read_text().isdigit():
                        break
                    time.sleep(0.01)
                self.assertTrue(
                    child_pid_path.exists() and child_pid_path.read_text().isdigit(),
                    "descendant did not start",
                )
                os.kill(os.getpid(), signal.SIGTERM)
                self.fail("SIGTERM was not handled")

            previous = signal.getsignal(signal.SIGTERM)
            with contextlib.redirect_stdout(io.StringIO()), self.assertRaises(SystemExit) as stopped:
                macos_build.run_build(
                    [sys.executable, "-c", cargo_code], root / "reports", 0.05,
                    sampler=cancel_when_ready,
                )
            self.assertEqual(stopped.exception.code, 128 + signal.SIGTERM)
            self.assertEqual(signal.getsignal(signal.SIGTERM), previous)
            child_pid = int(child_pid_path.read_text())
            for pid in (pids[0], child_pid):
                deadline = time.monotonic() + 5
                while True:
                    status = subprocess.run(
                        ["ps", "-p", str(pid), "-o", "stat="], capture_output=True, text=True,
                    ).stdout.strip()
                    if not status or status.startswith("Z"):
                        break
                    if time.monotonic() >= deadline:
                        self.fail(f"build process {pid} survived cancellation: {status}")
                    time.sleep(0.01)


if __name__ == "__main__":
    unittest.main()
