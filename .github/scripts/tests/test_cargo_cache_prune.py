"""Keep saved Cargo caches limited to third-party outputs on every platform."""

import contextlib
import importlib.util
import io
import json
from pathlib import Path
import re
import sys
import tempfile
import unittest


sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
SCRIPT = ROOT / ".github/scripts/prune-cargo-workspace-artifacts.py"
spec = importlib.util.spec_from_file_location("prune_cargo", SCRIPT)
prune_cargo = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prune_cargo)

H1, H2 = "0123456789abcdef", "fedcba9876543210"
METADATA = {
    "workspace_members": ["path+file:///repo#xai-grok-pager@1.0.0", "path+file:///repo#grok-zh@1.0.0",
                          "path+file:///repo#c@0.1.0"],
    "packages": [
        {"id": "path+file:///repo#xai-grok-pager@1.0.0", "name": "xai-grok-pager", "targets": [
            {"name": "xai_grok_pager", "kind": ["lib"]},
            {"name": "build-script-build", "kind": ["custom-build"]},
            {"name": "render-test", "kind": ["test"]},
        ]},
        {"id": "path+file:///repo#grok-zh@1.0.0", "name": "grok-zh", "targets": [
            {"name": "grok-zh", "kind": ["bin"]},
        ]},
        # A tiny workspace crate whose name is a suffix of a third-party crate (libc).
        {"id": "path+file:///repo#c@0.1.0", "name": "c", "targets": [{"name": "c", "kind": ["lib"]}]},
        {"id": "registry+https://x#serde@1.0.0", "name": "serde", "targets": [
            {"name": "serde", "kind": ["lib"]}]},
    ],
}

WORKSPACE_FILES = [
    f".fingerprint/xai-grok-pager-{H1}/lib-xai_grok_pager",
    f".fingerprint/grok-zh-{H2}/bin-grok-zh",
    f"build/xai-grok-pager-{H1}/build_script_build-{H1}",
    f"build/xai-grok-pager-{H2}/out/generated.rs",
    f"deps/libxai_grok_pager-{H1}.rlib",
    f"deps/libxai_grok_pager-{H1}.rmeta",
    f"deps/xai_grok_pager-{H1}.d",
    f"deps/render_test-{H1}.exe",
    f"deps/grok_zh-{H2}.exe",
    f"deps/grok_zh-{H2}.pdb",
    f"deps/grok_zh-{H2}.dSYM/Contents/Info.plist",
    f"deps/libc-{H1}.rlib",
    "incremental/xai_grok_pager-1abc2def3gh4i/s-x/dep-graph.bin",
    "grok-zh", "grok-zh.exe", "grok-zh.d", "grok_zh.pdb", "libxai_grok_pager.rlib", "libxai_grok_pager.d",
]
THIRD_PARTY_FILES = [
    ".cargo-lock", ".rustc_info.json",
    f".fingerprint/serde-{H1}/lib-serde",
    f".fingerprint/libc-{H2}/lib-libc",
    f".fingerprint/xai-grok-pager-extra-{H1}/lib",
    f"build/serde-{H1}/build_script_build-{H1}",
    f"build/libc-{H2}/out/x",
    f"deps/libserde-{H1}.rlib",
    f"deps/serde-{H1}.d",
    f"deps/liblibc-{H2}.rlib",
    f"deps/libc-{H2}.d",
    f"deps/libxai_grok_pager_extra-{H1}.rlib",
    f"deps/libxai_grok_pager-{H1}.rlib.tmp/keep",
    "incremental/serde-1abc2def3gh4i/s-x/dep-graph.bin",
    "build_script_build.d",
]


class PruneTests(unittest.TestCase):
    def make_profile(self, root):
        for name in WORKSPACE_FILES + THIRD_PARTY_FILES:
            path = root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"x" * 10)
        return root

    def test_removes_only_workspace_outputs_and_keeps_third_party_hits(self):
        workspace = prune_cargo.WorkspaceNames.from_metadata(METADATA)
        self.assertNotIn("serde", workspace.packages)
        self.assertNotIn("build_script_build", workspace.crates)
        with tempfile.TemporaryDirectory() as folder:
            profile = self.make_profile(Path(folder) / "release-dist")
            report = prune_cargo.prune([profile, Path(folder) / "missing"], workspace)
            for name in WORKSPACE_FILES:
                top = profile / Path(name).parts[0]
                if len(Path(name).parts) > 1:
                    top = profile / Path(name).parts[0] / Path(name).parts[1]
                with self.subTest(removed=name):
                    self.assertFalse(top.exists())
            for name in THIRD_PARTY_FILES:
                with self.subTest(kept=name):
                    self.assertTrue((profile / name).exists())
            self.assertEqual(report[0]["entries"], len(WORKSPACE_FILES))
            self.assertEqual(report[0]["after"], len(THIRD_PARTY_FILES) * 10)
            self.assertFalse(report[1]["exists"])

    def test_dry_run_reports_without_deleting(self):
        workspace = prune_cargo.WorkspaceNames.from_metadata(METADATA)
        with tempfile.TemporaryDirectory() as folder:
            profile = self.make_profile(Path(folder) / "debug")
            report = prune_cargo.prune([profile], workspace, dry_run=True)
            self.assertGreater(report[0]["removed_total"], 0)
            for name in WORKSPACE_FILES:
                self.assertTrue((profile / name).exists())

    def test_cli_uses_metadata_and_fails_loudly_without_members(self):
        with tempfile.TemporaryDirectory() as folder:
            folder = Path(folder)
            profile = self.make_profile(folder / "release-dist")
            good = folder / "metadata.json"
            good.write_text(json.dumps(METADATA), encoding="utf-8")
            with contextlib.redirect_stdout(io.StringIO()) as out:
                self.assertEqual(prune_cargo.main(["--metadata-json", str(good), "--profile-dir", str(profile)]), 0)
            self.assertIn("本仓 workspace：3 个包", out.getvalue())
            self.assertFalse((profile / "grok-zh").exists())
            with contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(prune_cargo.main(["--metadata-json", str(good), "--profile-dir-list",
                                                   f"{profile}\r\n\n  {folder / 'other'}  \n"]), 0)
            with contextlib.redirect_stderr(io.StringIO()) as err:
                self.assertEqual(prune_cargo.main(["--metadata-json", str(good), "--profile-dir-list", "\n "]), 1)
            self.assertIn("未提供", err.getvalue())

            empty = folder / "empty.json"
            empty.write_text(json.dumps({"workspace_members": [], "packages": []}), encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()) as err:
                self.assertEqual(prune_cargo.main(["--metadata-json", str(empty), "--profile-dir", str(profile)]), 1)
            self.assertIn("::error::", err.getvalue())
            with contextlib.redirect_stderr(io.StringIO()) as err:
                code = prune_cargo.main(["--cargo", str(folder / "no-such-cargo"), "--profile-dir", str(profile)])
            self.assertEqual(code, 1)
            self.assertIn("拒绝保存未清理的缓存", err.getvalue())

    def test_workspace_names_come_from_cargo_metadata_not_a_fixed_list(self):
        source = SCRIPT.read_text(encoding="utf-8")
        self.assertIn('"metadata", "--format-version", "1", "--no-deps", "--offline"', source)
        self.assertNotRegex(source, r'"xai[-_]grok')


class WorkflowContractTests(unittest.TestCase):
    ACTIONS = ("build-linux-x64", "build-macos-arm", "build-windows-arm", "build-windows-msvc-x64",
               "build-windows-msvc-cross", "build-windows-gnu-cross", "validate-windows-gnu")

    def save_blocks(self, text):
        blocks = re.split(r"\n\s*- (?:name|uses):", text)
        return [b for b in blocks if "uses: ./.github/actions/save-cargo-cache" in b
                or b.lstrip().startswith("./.github/actions/save-cargo-cache")]

    def test_every_target_cache_is_pruned_and_dependency_caches_are_not(self):
        for name in self.ACTIONS:
            text = (ROOT / f".github/actions/{name}/action.yml").read_text(encoding="utf-8")
            blocks = self.save_blocks(text)
            self.assertTrue(blocks, name)
            pruned = 0
            for block in blocks:
                path = block.split("path:", 1)[1].split("key:", 1)[0]
                dirs = [line.strip() for line in path.replace("|", "").splitlines() if line.strip()]
                with self.subTest(action=name, paths=dirs):
                    if any("target" in d for d in dirs):
                        prune = block.split("prune-profile-dirs: |", 1)[1]
                        listed = [line.strip() for line in prune.splitlines() if line.strip()]
                        self.assertEqual(listed[:len(dirs)], dirs)
                        pruned += 1
                    else:
                        self.assertNotIn("prune-profile-dirs", block)
            self.assertEqual(pruned, 1, name)

    def test_prune_runs_on_every_os_only_before_a_real_save(self):
        action = (ROOT / ".github/actions/save-cargo-cache/action.yml").read_text(encoding="utf-8")
        lookup = action.index("lookup-only: true")
        unix = action.index("清理本仓 crate 产物（Linux/macOS）")
        windows = action.index("清理本仓 crate 产物（Windows）")
        save = action.index("actions/cache/save@")
        self.assertLess(lookup, unix)
        self.assertLess(unix, save)
        self.assertLess(windows, save)
        for marker, shell, os_check in (("（Linux/macOS）", "shell: bash", "runner.os != 'Windows'"),
                                        ("（Windows）", "shell: pwsh", "runner.os == 'Windows'")):
            step = action.split("清理本仓 crate 产物" + marker, 1)[1].split("- name:", 1)[0]
            with self.subTest(step=marker):
                self.assertIn(shell, step)
                self.assertIn(os_check, step)
                self.assertIn("steps.existing.outputs.cache-hit != 'true'", step)
                self.assertIn("prune-cargo-workspace-artifacts.py --profile-dir-list", step)
                self.assertNotIn("continue-on-error", step)
        self.assertIn("$LASTEXITCODE -ne 0", action)


if __name__ == "__main__":
    unittest.main()
