"""Offline exact-artifact, provenance, corruption and acceptance CI contracts."""
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import tempfile
import unittest
import zipfile

import test_embedded_msvc_bundle as embedded_tests
import test_migration_package
from test_preview_workflow import step_blocks, upload_steps
from test_release_workflow import job_blocks

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location(
    "windows_acceptance_builder", ROOT / ".github/scripts/build-windows-acceptance-bundle.py")
acceptance = importlib.util.module_from_spec(spec)
spec.loader.exec_module(acceptance)
embedded = embedded_tests.builder
VERSION = "1.0.99"
COMMIT = "74c5269942d9a47bf09a9268db0bb5c145967e27"
REPOSITORY = "JoyElliot/grok-build-Chinese"


class AcceptanceBundleTests(unittest.TestCase):
    def fixture(self, root, *, commit=COMMIT, repository=REPOSITORY):
        payload, output = test_migration_package.MigrationPackageTests().fixture(root)
        (output / "grok-zh.exe").unlink()
        (payload / "grok-zh.exe").write_bytes(embedded_tests.fake_pe_prefix())
        info = payload / "BUILD-INFO.txt"
        info.write_text(info.read_text(encoding="utf-8") +
                        f"Commit: {commit}\nRepository: {repository}\n", encoding="utf-8")
        test_migration_package.MigrationPackageTests().rehash(payload)
        native = embedded.native_archive_from_directory(output, payload, VERSION)
        prefix = root / "prefix.exe"
        prefix.write_bytes(embedded_tests.fake_pe_prefix())
        embedded.bundle.append_bundle(prefix, native, output / "grok-zh.exe", VERSION)
        gnu = embedded.build_compat_package(output, payload, VERSION, archive=native)
        kit = root / "kit"
        kit.mkdir()
        for name in acceptance.KIT_FILES:
            (kit / name).write_bytes(("fixture " + name + "\n").encode("utf-8-sig"))
        return payload, native, gnu, kit

    def build(self, root, native, gnu, kit, **changes):
        arguments = dict(output=root / "acceptance", native_archive=native,
                         embedded_gnu_archive=gnu, version=VERSION, commit=COMMIT,
                         repository=REPOSITORY, run_id="123456", run_attempt="2", kit_directory=kit)
        arguments.update(changes)
        return acceptance.build_bundle(**arguments)

    def test_flat_bundle_contains_exact_archives_and_only_allowlisted_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, native, gnu, kit = self.fixture(root)
            (kit / "private.txt").write_text("not shipped", encoding="utf-8")
            (kit / "compiler.exe").write_bytes(b"not shipped")
            (kit / "nested").mkdir()
            (kit / "nested/anything.py").write_bytes(b"not shipped")
            provenance = self.build(root, native, gnu, kit)
            output = root / "acceptance"
            expected = {native.name, gnu.name, acceptance.PROVENANCE, acceptance.MANIFEST,
                        *acceptance.KIT_FILES}
            self.assertEqual({path.name for path in output.iterdir()}, expected)
            self.assertTrue(all(path.is_file() and not path.is_symlink() for path in output.iterdir()))
            self.assertEqual((output / native.name).read_bytes(), native.read_bytes())
            self.assertEqual((output / gnu.name).read_bytes(), gnu.read_bytes())
            for name in acceptance.KIT_FILES:
                self.assertEqual((output / name).read_bytes(), (kit / name).read_bytes())
            self.assertEqual(json.loads((output / acceptance.PROVENANCE).read_text()), provenance)

    def test_manifest_covers_scripts_readme_provenance_and_archives_except_itself(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, native, gnu, kit = self.fixture(root)
            self.build(root, native, gnu, kit)
            output = root / "acceptance"
            hashes = dict(line.split("  ")[::-1] for line in
                          (output / acceptance.MANIFEST).read_text(encoding="ascii").splitlines())
            self.assertEqual(set(hashes), {native.name, gnu.name, acceptance.PROVENANCE, *acceptance.KIT_FILES})
            for name, digest in hashes.items():
                self.assertEqual(digest, hashlib.sha256((output / name).read_bytes()).hexdigest())
                original = (output / name).read_bytes()
                (output / name).write_bytes(original + b"tamper")
                self.assertNotEqual(digest, hashlib.sha256((output / name).read_bytes()).hexdigest())
                (output / name).write_bytes(original)

    def test_provenance_binds_actual_run_version_targets_hashes_and_limited_scope(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, native, gnu, kit = self.fixture(root)
            result = self.build(root, native, gnu, kit)
            self.assertEqual(set(result), {"schema", "version", "commit", "repository", "run_id",
                                          "run_attempt", "run_url", "scope", "archives"})
            self.assertEqual(result["schema"], 1)
            self.assertEqual(result["version"], VERSION)
            self.assertEqual(result["commit"], COMMIT)
            self.assertEqual(result["repository"], REPOSITORY)
            self.assertEqual((result["run_id"], result["run_attempt"]), ("123456", "2"))
            self.assertEqual(result["run_url"], f"https://github.com/{REPOSITORY}/actions/runs/123456/attempts/2")
            for role, path, target in (("native", native, "x86_64-pc-windows-msvc"),
                                       ("embedded_gnu", gnu, "x86_64-pc-windows-gnu")):
                descriptor = result["archives"][role]
                self.assertEqual(descriptor["file"], path.name)
                self.assertEqual(descriptor["sha256"], hashlib.sha256(path.read_bytes()).hexdigest())
                self.assertEqual(descriptor["size"], path.stat().st_size)
                self.assertEqual(descriptor["target"], target)
            self.assertEqual(result["archives"]["embedded_gnu"]["embedded_native_sha256"],
                             result["archives"]["native"]["sha256"])
            self.assertEqual(result["archives"]["embedded_gnu"]["embedded_native_size"], native.stat().st_size)
            self.assertTrue(result["scope"]["preview_only"])
            self.assertIn("embedded-offline-migration", result["scope"]["automated_checks"])
            self.assertTrue({"interactive-tui-rendering", "login-and-account", "online-update",
                             "release-approval", "historical-client-download", "two-version-update",
                             "downgrade", "antivirus-low-disk-interruption"} <=
                            set(result["scope"]["excluded_checks"]))

    def test_invalid_identity_fails_before_creating_output(self):
        changes = [{"version": "1.0.99-preview"}, {"commit": "abc"}, {"commit": COMMIT.upper()},
                   {"repository": "../repo"}, {"repository": "owner/repo\n"},
                   {"run_id": "0"}, {"run_id": "0123"}, {"run_attempt": "-1"},
                   {"run_attempt": "2/extra"}]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, native, gnu, kit = self.fixture(root)
            for change in changes:
                with self.subTest(change=change), self.assertRaises(ValueError):
                    self.build(root, native, gnu, kit, **change)
                self.assertFalse((root / "acceptance").exists())
                self.assertFalse(list(root.glob(".grok-acceptance-*")))

    def test_native_source_commit_and_repository_must_match_provenance(self):
        for fields in ({"commit": "0" * 40}, {"repository": "elsewhere/repo"},
                       {"commit": COMMIT + "\nCommit: " + COMMIT}):
            with self.subTest(fields=fields), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                _, native, gnu, kit = self.fixture(root, **fields)
                with self.assertRaisesRegex(ValueError, "does not match provenance"):
                    self.build(root, native, gnu, kit)
                self.assertFalse((root / "acceptance").exists())

    def test_valid_but_different_native_zip_is_not_the_embedded_payload(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            payload, native, gnu, kit = self.fixture(root)
            (payload / "future-resource.dat").write_bytes(b"a different authenticated native package")
            test_migration_package.MigrationPackageTests().rehash(payload)
            native.unlink()
            embedded.native_archive_from_directory(native.parent, payload, VERSION)
            with self.assertRaisesRegex(ValueError, "exact supplied native ZIP"):
                self.build(root, native, gnu, kit)
            self.assertFalse((root / "acceptance").exists())

    def test_both_archive_validators_reject_corruption_and_unsafe_members(self):
        for target in ("native", "gnu"):
            for mutation in ("corrupt", "unsafe", "changed-member"):
                with self.subTest(target=target, mutation=mutation), tempfile.TemporaryDirectory() as directory:
                    root = Path(directory)
                    _, native, gnu, kit = self.fixture(root)
                    path = native if target == "native" else gnu
                    if mutation == "corrupt":
                        path.write_bytes(b"not a ZIP")
                    elif mutation == "unsafe":
                        with zipfile.ZipFile(path, "a") as zipped:
                            zipped.writestr(path.stem + "/../escape.txt", b"escape")
                    else:
                        with zipfile.ZipFile(path) as zipped:
                            entries = [(entry, zipped.read(entry)) for entry in zipped.infolist()]
                        with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as zipped:
                            for entry, data in entries:
                                zipped.writestr(entry, b"tamper" if entry.filename.endswith("rg.exe") else data)
                    with self.assertRaises((ValueError, zipfile.BadZipFile)):
                        self.build(root, native, gnu, kit)
                    self.assertFalse((root / "acceptance").exists())
                    self.assertFalse((root / "escape.txt").exists())

    def test_missing_or_symlink_kit_file_and_wrong_archive_name_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, native, gnu, kit = self.fixture(root)
            renamed = native.with_name("unreviewed.zip")
            renamed.write_bytes(native.read_bytes())
            with self.assertRaisesRegex(ValueError, "canonical"):
                self.build(root, native, gnu, kit, native_archive=renamed)
            script = kit / acceptance.KIT_FILES[0]
            script.unlink()
            with self.assertRaisesRegex(ValueError, "regular file"):
                self.build(root, native, gnu, kit)
            self.assertFalse((root / "acceptance").exists())
            try:
                script.symlink_to(kit / acceptance.KIT_FILES[1])
            except OSError:
                self.skipTest("platform cannot create an unprivileged symlink")
            with self.assertRaisesRegex(ValueError, "regular file"):
                self.build(root, native, gnu, kit)
            self.assertFalse((root / "acceptance").exists())

    def test_existing_output_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, native, gnu, kit = self.fixture(root)
            (root / "acceptance").mkdir()
            sentinel = root / "acceptance/keep.txt"
            sentinel.write_bytes(b"keep")
            with self.assertRaises(FileExistsError):
                self.build(root, native, gnu, kit)
            self.assertEqual(sentinel.read_bytes(), b"keep")
            self.assertEqual(list(sentinel.parent.iterdir()), [sentinel])

    def test_archive_symlink_is_rejected_without_reading_through_it(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, native, gnu, kit = self.fixture(root)
            preserved = native.with_suffix(".original")
            native.rename(preserved)
            try:
                native.symlink_to(preserved)
            except OSError:
                self.skipTest("platform cannot create an unprivileged symlink")
            with self.assertRaisesRegex(ValueError, "regular file"):
                self.build(root, native, gnu, kit)
            self.assertFalse((root / "acceptance").exists())

    def test_empty_or_oversized_kit_file_is_rejected_atomically(self):
        for content in (b"", b"x" * (acceptance.MAX_KIT_FILE_BYTES + 1)):
            with self.subTest(size=len(content)), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                _, native, gnu, kit = self.fixture(root)
                (kit / acceptance.KIT_FILES[0]).write_bytes(content)
                with self.assertRaisesRegex(ValueError, "regular file"):
                    self.build(root, native, gnu, kit)
                self.assertFalse((root / "acceptance").exists())
                self.assertFalse(list(root.glob(".grok-acceptance-*")))


class AcceptanceWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.workflow = (ROOT / ".github/workflows/zh-dev-windows-preview.yml").read_text(encoding="utf-8")
        cls.jobs = job_blocks(cls.workflow.split("jobs:\n", 1)[1])
        cls.job = cls.jobs["windows-msvc-migration-experiment"]
        cls.steps = step_blocks(cls.job)

    def by_id(self, identity):
        return next(step for step in self.steps if f"id: {identity}\n" in step)

    def test_branch_trigger_is_exact_and_regression_suite_is_in_preflight(self):
        push = self.workflow.split("  push:\n", 1)[1].split("  pull_request:", 1)[0]
        branches = push.split("    branches:\n", 1)[1].split("    paths-ignore:", 1)[0]
        self.assertEqual([line.strip().removeprefix("- ").strip("'") for line in branches.splitlines()],
                         ["zh-dev", "sync/**", "codex/free-account-localization-macos-smoke",
                          "test/windows-msvc-acceptance-bundle"])
        self.assertIn("python3 .github/scripts/tests/test_windows_acceptance_bundle.py", self.jobs["localization"])
        self.assertNotIn("contents: write", self.workflow)
        self.assertNotIn("release_build: 'true'", self.workflow)
        self.assertNotIn("GROK_RELEASE_BUILD:", self.workflow)

    def test_bundle_uses_exact_actual_zip_after_both_existing_migration_checks(self):
        self.assertLess(self.job.index("package-windows-msvc-x64.ps1"),
                        self.job.index("build-windows-embedded-bootstrap.py"))
        self.assertEqual(self.job.count("Test-EmbeddedMigrationProduct.ps1"), 2)
        self.assertLess(self.job.rindex("Test-EmbeddedMigrationProduct.ps1"),
                        self.job.index("build-windows-acceptance-bundle.py"))
        build = self.by_id("acceptance_bundle")
        for flag in ("--native-archive \"dist/$env:ARCHIVE_NAME\"",
                     "--embedded-gnu-archive \"migration-dist/grok-zh-$env:GROK_VERSION-windows-x86_64-gnu.zip\"",
                     "--commit $env:GITHUB_SHA", "--repository $env:GITHUB_REPOSITORY",
                     "--run-id $env:GITHUB_RUN_ID", "--run-attempt $env:GITHUB_RUN_ATTEMPT"):
            self.assertIn(flag, build)
        self.assertNotIn("continue-on-error", self.job)

    def test_shipped_script_runs_in_both_shells_without_python_or_compiler_path(self):
        for identity in ("acceptance_pwsh", "acceptance_windows_ps"):
            step = self.by_id(identity)
            self.assertIn("Join-Path $bundle 'Run-WindowsAcceptance.ps1'", step)
            self.assertIn("-BundleDirectory $bundle -ResultsDirectory $results", step)
            self.assertIn("New-Item -ItemType Directory -Path $results,$console", step)
            self.assertIn('$env:PATH = "$env:SystemRoot\\System32;$env:SystemRoot;$env:SystemRoot\\System32\\WindowsPowerShell\\v1.0"', step)
            self.assertIn("@('python', 'python3', 'gcc', 'cl', 'cargo', 'rustc')", step)
            self.assertIn("if ($code -ne 0)", step)
            self.assertNotIn("-InteractiveTui", step)
        self.assertIn("Get-Command pwsh.exe", self.by_id("acceptance_pwsh"))
        legacy = self.by_id("acceptance_windows_ps")
        self.assertIn("System32/WindowsPowerShell/v1.0/powershell.exe", legacy)
        self.assertIn("always() && steps.acceptance_bundle.outcome == 'success'", legacy)

    def test_download_requires_both_shell_success_and_failure_diagnostics_are_bounded(self):
        upload = self.by_id("acceptance_artifact")
        self.assertIn("success() && steps.acceptance_pwsh.outcome == 'success' && steps.acceptance_windows_ps.outcome == 'success'", upload)
        self.assertIn("path: ${{ runner.temp }}/windows-acceptance\n", upload)
        self.assertIn("retention-days: 14", upload)
        self.assertIn("if-no-files-found: error", upload)
        diagnostics = next(step for step in upload_steps(self.job) if "name: windows-acceptance-diagnostics-" in step)
        self.assertIn("if: always()", diagnostics)
        self.assertIn("windows-acceptance-results/**/logs/**", diagnostics)
        self.assertIn("windows-acceptance-results/**/RESULT.json", diagnostics)
        self.assertIn("windows-acceptance-console/*.log", diagnostics)
        self.assertNotRegex(diagnostics, r"windows-acceptance-results/(?:\*\*)?\s*$")
        self.assertIn("if-no-files-found: warn", diagnostics)
        self.assertLess(self.job.index("id: acceptance_windows_ps"), self.job.index("id: acceptance_artifact"))
        self.assertNotIn("gh release", self.job)


class AcceptanceScriptPackagingTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.kit = ROOT / "packaging/windows/acceptance"
        cls.scripts = {name: (cls.kit / name).read_text(encoding="utf-8-sig")
                       for name in acceptance.KIT_FILES if name.endswith(".ps1")}
        cls.runner = cls.scripts["Run-WindowsAcceptance.ps1"]
        cls.readme = (cls.kit / "README.zh-CN.md").read_text(encoding="utf-8")

    def function(self, name):
        match = re.search(r"(?ms)^function " + re.escape(name) + r"\([^\n]*\) \{\n.*?^\}", self.runner)
        self.assertIsNotNone(match, "missing acceptance helper: " + name)
        return match[0]

    def test_powershell_51_encoding_and_explicit_wrapper_dependency_are_shipped(self):
        for name, text in self.scripts.items():
            with self.subTest(name=name):
                self.assertTrue((self.kit / name).read_bytes().startswith(b"\xef\xbb\xbf"))
                self.assertRegex(text, r"\A#requires -Version 5\.1\r?\n")
        wrapper = self.scripts["Start-IsolatedTui.ps1"]
        dependencies = re.findall(r"Join-Path \$PSScriptRoot '([^']+\.ps1)'", wrapper)
        self.assertEqual(dependencies, ["Run-WindowsAcceptance.ps1"])
        self.assertTrue(all(name in acceptance.KIT_FILES and (self.kit / name).is_file()
                            for name in dependencies))
        self.assertIn("-InteractiveTui", wrapper)
        # Building with the actual checked-in kit catches a missing README or
        # accidental reliance on another source-tree script before Windows CI.
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, native, gnu, _ = AcceptanceBundleTests().fixture(root)
            AcceptanceBundleTests().build(root, native, gnu, self.kit)
            for name in acceptance.KIT_FILES:
                self.assertEqual((root / "acceptance" / name).read_bytes(), (self.kit / name).read_bytes())

    def test_user_entrypoints_do_not_run_build_tools_download_code_or_delete_trees(self):
        forbidden_commands = (r"python(?:3)?(?:\.exe)?", r"gcc(?:\.exe)?", r"cargo(?:\.exe)?",
                              r"rustc(?:\.exe)?", "Remove-Item", "Invoke-WebRequest",
                              "Invoke-RestMethod", "Install-Module", "Set-ExecutionPolicy")
        command = r"(?im)(?:^\s*|[;|&]\s*)(?:&\s*)?(?:" + "|".join(forbidden_commands) + r")\b"
        for name, text in self.scripts.items():
            with self.subTest(name=name):
                self.assertNotRegex(text, command)
                self.assertNotRegex(text, r"(?i)\[(?:IO\.)?(?:File|Directory)\]::Delete\s*\(")
        paths = self.function("Assert-AcceptancePath")
        self.assertRegex(paths, r"ReparsePoint\)\s*-eq\s*0")
        for destination in ("$Directory", "$resultParent", "$Destination"):
            self.assertIn("Assert-AcceptancePath " + destination, self.runner)

    def test_only_product_installer_mutation_is_explicit_isolated_portable_install(self):
        invocations = re.findall(r"(?m)^\s*([&.])\s+\(Join-Path (\$\w+) 'Install-GrokZh\.ps1'\)([^\n]*)", self.runner)
        # In-process loading is verifier-only. Installation and its EXE probes
        # must use the scrubbed child-process path, never inherit the host shell.
        self.assertEqual(len(invocations), 1)
        self.assertEqual(invocations[0][:2], (".", "$nativeRoot"))
        self.assertRegex(invocations[0][2], r"^\s+-LibraryOnly\s*$")
        command = re.search(r'(?m)^\s*\$installCommand\s*=\s*"([^"\n]+)"', self.runner)
        self.assertIsNotNone(command)
        for flag in (r"&\s+'\$installerPath'", r"-PackageDir\s+'\$packageArgument'",
                     r"-InstallDir\s+'\$installArgument'", r"-GrokHome\s+'\$dataArgument'",
                     r"-NoPathUpdate\b", r"-OverrideOfficialCommands:`\$false\b"):
            self.assertRegex(command[1], flag)
        self.assertIn("$installerPath = (Join-Path $gnuRoot 'Install-GrokZh.ps1')", self.runner)
        for argument, source in (("packageArgument", "gnuRoot"), ("installArgument", "portableDirectory"),
                                 ("dataArgument", "dataDirectory")):
            self.assertIn(f'${argument} = ${source}.Replace("\'", "\'\'")', self.runner)
        self.assertIn("[Text.Encoding]::Unicode.GetBytes($installCommand)", self.runner)
        self.assertRegex(self.runner, r"Invoke-AcceptanceCommand \(Join-Path \$PSHOME \$shellName\) [^\n]+-EncodedCommand \$encodedCommand")
        self.assertRegex(self.runner, r"\$portableDirectory\s*=\s*Join-Path \$sandbox 'portable-install'")
        self.assertRegex(self.runner, r"\$dataDirectory\s*=\s*Join-Path \$sandbox 'shared-home'")

    def test_child_environment_is_cleared_and_interactive_home_is_separate(self):
        process = self.function("New-AcceptanceProcess")
        self.assertIn("$start.UseShellExecute = $false", process)
        self.assertIn("$start.WorkingDirectory = $script:AcceptanceWorkspace", process)
        clear_at = process.index("$start.EnvironmentVariables.Clear()")
        assign_at = process.index("$start.EnvironmentVariables[$name] =")
        self.assertLess(clear_at, assign_at)
        # The allowlist may grow only after review: provider keys, tokens and
        # user profile/configuration variables must never be copied wholesale.
        names = re.findall(r"'([^']+)'", process.split("foreach ($name in @(", 1)[1].split(") {", 1)[0])
        self.assertEqual(set(names), {"SystemRoot", "WINDIR", "ComSpec", "PATHEXT", "ProgramFiles",
                                     "ProgramFiles(x86)", "ProgramData", "OS"})
        self.assertRegex(self.runner, r"GROK_HOME\s*=\s*\$dataDirectory\b")
        self.assertRegex(self.runner, r"\$interactiveHome\s*=\s*Join-Path \$sandbox 'interactive-home'")
        self.assertIn("$script:AcceptanceEnvironment.GROK_HOME=$interactiveHome", self.runner)
        self.assertIn("if ($InteractiveTui)", self.runner)

    def test_readme_explains_scope_credentials_and_uncovered_upgrade_cases(self):
        section = self.readme.split("## 仍然未覆盖", 1)[1]
        for topic in ("真实历史客户端", "连续两次版本更新", "第二版本", "降级", "真实账号",
                      "TUI", "低磁盘空间", "杀毒", "中断恢复"):
            self.assertIn(topic, section)
        self.assertIn("不是发布签名", self.readme)
        self.assertIn("不是操作系统安全隔离机制", self.readme)
        self.assertIn("不会运行更新", self.readme)
        self.assertIn("interactive-home", self.readme)
        self.assertIn("可能含私人凭据", self.readme)
        self.assertIn("不要上传整个沙盒", self.readme)


if __name__ == "__main__":
    unittest.main()
