"""Check cache invalidation and fallback without compiling the application."""

import contextlib
import hashlib
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock
import zipfile

from test_release_workflow import job_blocks

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location("windows_rust_cache", ROOT / ".github/scripts/windows-rust-cache.py")
CACHE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CACHE)


class WindowsRustCacheTests(unittest.TestCase):
    def test_resource_content_additions_removals_and_names_invalidate(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "catalog.json").write_text("first")
            initial = CACHE.resource_fingerprint(root, ["catalog.json"])
            (root / "catalog.json").write_text("second")
            self.assertNotEqual(initial, CACHE.resource_fingerprint(root, ["catalog.json"]))
            (root / "new.json").write_text("second")
            current = CACHE.resource_fingerprint(root, ["catalog.json"])
            self.assertNotEqual(current, CACHE.resource_fingerprint(root, ["catalog.json", "new.json"]))
            self.assertNotEqual(current, CACHE.resource_fingerprint(root, []))
            self.assertNotEqual(current, CACHE.resource_fingerprint(root, ["new.json"]))

    def test_rust_sources_stay_with_rustc_dep_info_not_global_resource_hash(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "lib.rs").write_text("pub fn value() {}")
            first = CACHE.resource_fingerprint(root, ["lib.rs"])
            (root / "lib.rs").write_text("pub fn other() {}")
            self.assertEqual(first, CACHE.resource_fingerprint(root, ["lib.rs"]))

    def test_archive_hash_failure_never_extracts_or_executes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / CACHE.ARCHIVE).write_bytes(b"untrusted")
            with mock.patch.object(CACHE.subprocess, "check_output") as execute:
                with self.assertRaisesRegex(ValueError, "SHA-256"):
                    CACHE.install(root)
                execute.assert_not_called()
            self.assertFalse((root / "sccache.exe").exists())

    def test_only_pinned_executable_is_extracted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            archive = root / CACHE.ARCHIVE
            with zipfile.ZipFile(archive, "w") as bundle:
                bundle.writestr(f"sccache-v{CACHE.VERSION}-x86_64-pc-windows-msvc/sccache.exe", b"fixture")
                bundle.writestr("../unexpected", b"must not be extracted")
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            with mock.patch.object(CACHE, "ARCHIVE_SHA256", digest), mock.patch.object(
                CACHE.subprocess, "check_output", return_value=f"sccache {CACHE.VERSION}\n",
            ):
                self.assertEqual(CACHE.install(root).read_bytes(), b"fixture")
            self.assertEqual({p.name for p in root.iterdir()}, {CACHE.ARCHIVE, "sccache.exe"})

    def test_failed_startup_does_not_enable_wrapper(self):
        with tempfile.TemporaryDirectory() as directory:
            env = {"GROK_SCCACHE_EXE": "fake.exe", "GITHUB_ENV": str(Path(directory) / "env")}
            with mock.patch.dict(os.environ, env), mock.patch.object(
                CACHE.subprocess, "run", side_effect=subprocess.CalledProcessError(1, "sccache"),
            ):
                with self.assertRaises(subprocess.CalledProcessError):
                    CACHE.activate()
            self.assertFalse(Path(env["GITHUB_ENV"]).exists())

    def test_optional_setup_error_falls_back_but_does_not_run_cargo(self):
        with mock.patch("sys.argv", ["cache", "prepare"]), mock.patch.object(
            CACHE, "prepare", side_effect=OSError("offline"),
        ), mock.patch.object(CACHE.subprocess, "run") as command, contextlib.redirect_stdout(io.StringIO()) as output:
            CACHE.main()
            self.assertIn("Optional Rust compiler cache prepare unavailable", output.getvalue())
            command.assert_not_called()

    def test_stats_failure_still_stops_server(self):
        with tempfile.TemporaryDirectory() as directory:
            env = {"GROK_SCCACHE_ACTIVE": "true", "GROK_SCCACHE_EXE": "fake.exe", "CARGO_TARGET_DIR": directory}
            with mock.patch.dict(os.environ, env), mock.patch.object(
                CACHE.subprocess, "check_output", side_effect=OSError("stats unavailable"),
            ), mock.patch.object(CACHE.subprocess, "run") as stop:
                with self.assertRaises(OSError):
                    CACHE.finish()
                stop.assert_called_once_with(["fake.exe", "--stop-server"], timeout=20, check=True)

    def test_failed_stop_does_not_allow_cache_save(self):
        with tempfile.TemporaryDirectory() as directory:
            env = {"GROK_SCCACHE_ACTIVE": "true", "GROK_SCCACHE_EXE": "fake.exe", "CARGO_TARGET_DIR": directory}
            with mock.patch.dict(os.environ, env), mock.patch.object(
                CACHE.subprocess, "check_output", side_effect=['{}', 'stats'],
            ), mock.patch.object(CACHE.subprocess, "run", side_effect=subprocess.CalledProcessError(1, "stop")), mock.patch.object(CACHE, "publish") as publish:
                with self.assertRaises(subprocess.CalledProcessError):
                    CACHE.finish()
                publish.assert_not_called()

    def test_successful_stop_allows_cache_save(self):
        with tempfile.TemporaryDirectory() as directory:
            env = {"GROK_SCCACHE_ACTIVE": "true", "GROK_SCCACHE_EXE": "fake.exe", "CARGO_TARGET_DIR": directory, "GITHUB_ENV": str(Path(directory) / "env")}
            with mock.patch.dict(os.environ, env), mock.patch.object(
                CACHE.subprocess, "check_output", side_effect=['{}', 'stats'],
            ), mock.patch.object(CACHE.subprocess, "run"), contextlib.redirect_stdout(io.StringIO()):
                CACHE.finish()
                self.assertEqual(Path(env["GITHUB_ENV"]).read_text(), "GROK_SCCACHE_SAVE_READY=true\n")

    def test_release_read_only_and_original_cargo_commands_and_gates(self):
        for filename, job, mode in (("zh-dev-windows-preview.yml", "windows-gnu-build", "preview"),
                                    ("zh-release-windows.yml", "windows-x64-gnu", "release")):
            jobs = job_blocks((ROOT / ".github/workflows" / filename).read_text(encoding="utf-8"))
            build = jobs[job]
            for option in ("--profile release-dist", "--features release-dist", "--config profile.release-dist.debug=0"):
                self.assertIn(option, build)
            self.assertIn('CARGO_INCREMENTAL: "0"', build)
            self.assertNotIn("RUSTFLAGS:", build)
            if mode == "release":
                cache = build.split("uses: ./.github/actions/setup-windows-rust-cache", 1)[1].split("      - name:", 1)[0]
                self.assertIn("save-cache: 'false'", cache)
                self.assertNotIn("grok-rust-content-cache", build)
            else:
                save = build.split("- name: 保存有容量上限的 Rust 内容缓存", 1)[1]
                self.assertIn("success()", save)
                self.assertIn("env.GROK_SCCACHE_SAVE_READY == 'true'", save)
                self.assertIn("github.event.pull_request.head.repo.full_name == github.repository", save)
                self.assertIn("github.actor != 'dependabot[bot]'", save)
            cargo = build.split("cargo build ", 1)[1].split("      - name:", 1)[0]
            self.assertNotIn("continue-on-error:", cargo)
            self.assertTrue("throw" in cargo)
        action = (ROOT / ".github/actions/setup-windows-rust-cache/action.yml").read_text(encoding="utf-8")
        self.assertNotIn("upload-artifact", action)
        self.assertNotIn("github.sha", action)
        self.assertNotIn("github.run_id", action)
        self.assertLessEqual(CACHE.CACHE_BYTES, 256 * 1024 * 1024)


if __name__ == "__main__":
    unittest.main()
