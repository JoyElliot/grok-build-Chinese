"""Exercise preview gating and artifact retention without running CI or builds."""

import ast
import os
from pathlib import Path
import re
import subprocess
import tempfile
import textwrap
import unittest

from test_release_workflow import dependencies, job_blocks


ROOT = Path(__file__).resolve().parents[3]
WORKFLOW = ROOT / ".github/workflows/zh-dev-windows-preview.yml"
PREFLIGHT_JOBS = {"zh-dev-priority-gate", "localization", "rust-format-preflight"}
HEAVY_ROOT_JOBS = {
    "windows-gnu-rust-validation", "windows-gnu-cross-build", "native-rust-validation",
    "windows-arm64-msvc-preview", "windows-msvc-migration-test", "windows-msvc-cross-build",
    "linux-x64-gnu-preview", "linux-arm64-gnu-preview", "macos-arm-preview", "macos-intel-preview",
}
PACKAGE_JOBS = {
    "windows-arm64-msvc-preview": "build-windows-arm",
    "linux-x64-gnu-preview": "build-linux-x64",
    "linux-arm64-gnu-preview": "build-linux-x64",
    "macos-arm-preview": "build-macos-arm",
    "macos-intel-preview": "build-macos-arm",
}
# Keep every pre-merge lightweight test, not just the six runner commands.
ORIGINAL_TESTS = {
    ".github/scripts/tests/test_upstream_localization.py": (
        "test_upstream_changelogs_keep_each_entry_and_machine_field",
        "test_public_configuration_table_has_the_same_keys",
        "test_english_guides_link_to_the_shipped_chinese_counterparts",
        "test_new_owned_ui_messages_have_chinese_metadata",
        "test_literal_named_ui_keys_exist_in_a_chinese_catalog",
        "test_literal_key_guard_runs_for_source_only_changes",
        "test_shell_test_support_forwards_workspace_fixture_feature",
    ),
    ".github/scripts/tests/test_announcement_translations.py": (
        "test_current_catalog_and_empty_replacement",
        "test_published_catalog_cannot_be_rewritten_even_with_matching_hash",
        "test_published_catalog_cannot_be_deleted",
        "test_digest_mismatch_and_old_manifest_are_rejected",
        "test_contract_rejects_duplicates_links_controls_and_bad_versions",
        "test_newlines_are_message_only_and_fields_are_exact",
    ),
    ".github/scripts/tests/test_package_protocol.py": (
        "test_all_platforms_keep_the_same_package_members",
        "test_missing_and_mismatched_version_fail_before_writing",
    ),
    ".github/scripts/tests/test_macos_build.py": (
        "test_both_native_macos_targets_use_the_release_command",
        "test_thin_lto_trial_is_explicit_isolated_and_not_a_release_profile_switch",
        "test_formal_release_reuses_the_verified_preview_command_and_cache",
        "test_default_profile_keeps_current_optimization_settings",
        "test_only_trusted_preview_events_can_save_cache",
        "test_pull_requests_only_restore_caches",
        "test_configure_trial_from_environment_is_read_only_and_rejects_release",
        "test_process_samples_include_descendants_and_exclude_unrelated_compilers",
        "test_swap_units_and_missing_probes_are_not_reported_as_zero",
        "test_missing_root_and_empty_process_output_remain_unknown",
        "test_build_exit_code_and_diagnostics_survive_success_and_failure",
        "test_sigterm_stops_cargo_and_its_descendant",
    ),
    ".github/scripts/tests/test_cargo_cache_prune.py": (
        "test_removes_only_workspace_outputs_and_keeps_third_party_hits",
        "test_dry_run_reports_without_deleting",
        "test_cli_uses_metadata_and_fails_loudly_without_members",
        "test_workspace_names_come_from_cargo_metadata_not_a_fixed_list",
        "test_every_target_cache_is_pruned_and_dependency_caches_are_not",
        "test_linux_preview_saves_no_cache",
        "test_prune_runs_on_every_os_only_before_a_real_save",
    ),
    "packaging/macos/tests/test_install_lock.py": (
        "test_lock_outlives_perl_child_and_blocks_native_flock",
        "test_native_updater_lock_blocks_installer",
        "test_killed_installer_releases_lock_without_deleting_file",
        "test_symlink_is_rejected_without_touching_target",
        "test_insecure_existing_lock_is_rejected",
    ),
}


def condition(block):
    field = re.search(r"^    if: (.*)\n", block, re.M)
    if not field:
        return "true"
    if field[1] == ">-":
        continuation = re.match(r"(?:      .*\n)+", block[field.end():])
        if continuation is None:
            raise AssertionError("empty multiline job condition")
        return " ".join(continuation[0].split())
    return field[1]


def job_runs(block, results, event, ref):
    """Evaluate this workflow's restricted expressions, including implicit success()."""
    required = dependencies(block) if re.search(r"^    needs:", block, re.M) else set()
    expression = condition(block).replace("${{", "").replace("}}", "").strip()
    if not re.search(r"\b(?:always|success|failure|cancelled)\(\)", expression):
        if any(results[dep] != "success" for dep in required):
            return False
    context = {
        "github.event_name": event,
        "github.ref": ref,
        "github.repository": "JoyElliot/grok-build-Chinese",
    }
    for key, value in context.items():
        expression = expression.replace(key, repr(value))
    expression = expression.replace("always()", "True").replace("true", "True")
    expression = expression.replace("&&", " and ").replace("||", " or ")
    expression = re.sub(r"!(?!=)", " not ", expression).strip()
    tree = ast.parse(expression, mode="eval")
    allowed = (ast.Expression, ast.BoolOp, ast.And, ast.Or, ast.UnaryOp, ast.Not,
               ast.Compare, ast.Eq, ast.Constant, ast.Call, ast.Name, ast.Load)
    for node in ast.walk(tree):
        if not isinstance(node, allowed):
            raise AssertionError(f"unsupported gate expression: {ast.dump(node)}")
        if isinstance(node, ast.Name) and node.id != "startsWith":
            raise AssertionError(f"unsupported gate function: {node.id}")
    return eval(compile(tree, str(WORKFLOW), "eval"), {"__builtins__": {}},
                {"startsWith": lambda value, prefix: value.startswith(prefix)})


def step_blocks(text):
    """Read only top-level steps, never shell list items or nested action inputs."""
    indents = [len(match[1]) for match in re.finditer(r"^( +)- (?:name|uses):", text, re.M)]
    if not indents:
        raise AssertionError("no action/workflow steps")
    prefix = " " * min(indents)
    return re.split(r"^" + prefix + r"- ", text, flags=re.M)[1:]


def upload_steps(text):
    return [block for block in step_blocks(text)
            if "uses: actions/upload-artifact@" in block]


def step_runs(block, status="success", *, release=False, phase="build", event="push",
              report_dir="/runner/reports", previous_failure=False):
    """Evaluate actual diagnostic gates, including GitHub's implicit success()."""
    match = re.search(r"^ +if: (.*)$", block, re.M)
    expression = match[1] if match else "success()"
    expression = expression.replace("${{", "").replace("}}", "").strip()
    if not re.search(r"\b(?:always|success|failure|cancelled)\(\)", expression):
        if status != "success":
            return False
    context = {
        "inputs.release_build": "true" if release else "false",
        "inputs.mode": "release" if release else "preview",
        "inputs.phase": phase,
        "github.event_name": event,
        "env.MACOS_BUILD_REPORT_DIR": report_dir,
    }
    for key, value in context.items():
        expression = expression.replace(key, repr(value))
    statuses = {
        "always()": True,
        "success()": status == "success",
        "failure()": status == "failure" or previous_failure,
        "cancelled()": status == "cancelled",
    }
    for key, value in statuses.items():
        expression = expression.replace(key, repr(value))
    expression = expression.replace("&&", " and ").replace("||", " or ")
    expression = re.sub(r"!(?!=)", " not ", expression).strip()
    tree = ast.parse(expression, mode="eval")
    allowed = (ast.Expression, ast.BoolOp, ast.And, ast.Or, ast.UnaryOp, ast.Not,
               ast.Compare, ast.Eq, ast.NotEq, ast.Constant)
    if any(not isinstance(node, allowed) for node in ast.walk(tree)):
        raise AssertionError(f"unsupported step gate: {expression}")
    return bool(eval(compile(tree, str(WORKFLOW), "eval"), {"__builtins__": {}}, {}))


DIAGNOSTIC_ACTIONS = (
    "build-windows-gnu-cross", "build-windows-msvc-cross", "build-windows-msvc-x64",
    "build-windows-arm", "build-linux-x64", "build-macos-arm", "validate-windows-gnu",
)


def action_text(name):
    return (ROOT / f".github/actions/{name}/action.yml").read_text(encoding="utf-8")


def diagnostic_steps(text):
    return [step for step in upload_steps(text) if "if-no-files-found: warn" in step]


class PreviewWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = WORKFLOW.read_text(encoding="utf-8")
        cls.jobs = job_blocks(cls.text.split("jobs:\n", 1)[1])

    def test_original_lightweight_commands_and_all_39_test_methods_are_retained(self):
        localization = self.jobs["localization"]
        self.assertEqual(sum(map(len, ORIGINAL_TESTS.values())), 39)
        for path, expected in ORIGINAL_TESTS.items():
            with self.subTest(path=path):
                self.assertEqual(localization.count(f"python3 {path}"), 1)
                tree = ast.parse((ROOT / path).read_text(encoding="utf-8"))
                actual = {node.name for node in ast.walk(tree)
                          if isinstance(node, ast.FunctionDef) and node.name.startswith("test_")}
                self.assertTrue(set(expected).issubset(actual), set(expected) - actual)
        self.assertIn("python3 .github/scripts/tests/test_preview_workflow.py", localization)
        self.assertFalse((ROOT / ".github/workflows/upstream-localization.yml").exists())

    def test_custom_artifact_cleanup_is_removed_without_dangling_references(self):
        retired = (
            ".github/workflows/preview-retention.yml",
            ".github/scripts/preview-retention.py",
            ".github/scripts/tests/test_preview_retention.py",
        )
        for path in retired:
            with self.subTest(path=path):
                self.assertFalse((ROOT / path).exists())

        # Guard live automation and docs; tests may name retired files to assert absence.
        paths = list((ROOT / ".github/workflows").glob("*.yml"))
        paths += list((ROOT / ".github/actions").rglob("action.yml"))
        paths += [path for path in (ROOT / ".github/scripts").rglob("*")
                  if path.is_file() and "tests" not in path.parts
                  and "__pycache__" not in path.parts]
        paths += list((ROOT / "docs").glob("*.md"))
        paths += list(ROOT.glob("README*"))
        obsolete = (
            "preview-retention", "test_preview_retention.py", "retention-plan",
            "first_superseded_at", "--state-artifact-id", "首次替换时间",
            "完整性证明", "替换后 7 天", "替换后 14 天", "十四天宽限期",
        )
        for path in paths:
            text = path.read_text(encoding="utf-8")
            with self.subTest(path=path.relative_to(ROOT)):
                for marker in obsolete:
                    self.assertNotIn(marker, text)
        self.assertNotIn("actions: write", self.text)

    def test_docs_describe_fixed_native_expiry_without_custom_cleanup(self):
        doc = (ROOT / "docs/CI-PREVIEW-RETENTION.md").read_text(encoding="utf-8")
        self.assertIn("不再运行自定义制品清理", doc)
        self.assertIn("六平台预览包上传时设置 90 天", doc)
        self.assertIn("上传时设置 30 天", doc)
        self.assertIn("交叉编译中间包保留 7 天", doc)
        self.assertIn("独立 Windows MSVC 迁移实验产品仍为 14 天", doc)
        self.assertIn("原有到期时间", doc)
        self.assertNotIn("不受限制", doc)
        self.assertNotIn("无限", doc)

    def test_localization_is_early_read_only_and_does_not_wait_for_priority_gate(self):
        block = self.jobs["localization"]
        self.assertNotRegex(block, r"(?m)^    (?:if|needs):")
        self.assertIn("persist-credentials: false", block)
        self.assertIn("runs-on: ubuntu-24.04", block)
        self.assertIn("timeout-minutes: 10", block)
        self.assertNotIn("continue-on-error", block)
        self.assertNotIn("contents: write", self.text)
        self.assertLess(self.text.index("  localization:"), self.text.index("  rust-format-preflight:"))

    def test_existing_triggers_and_pr_concurrency_are_preserved_with_sync_coverage(self):
        triggers = self.text.split("\npermissions:", 1)[0]
        self.assertIn("      - 'sync/**'", triggers)
        self.assertIn("      - codex/free-account-localization-macos-smoke", triggers)
        self.assertIn("  pull_request:\n    branches:\n      - zh-dev", triggers)
        self.assertIn("- cron: '17 19 */5 * *'", triggers)
        self.assertIn("  workflow_dispatch:", triggers)
        self.assertNotIn("pull_request_target", triggers)
        self.assertNotRegex(triggers, r"(?m)^    paths:")
        self.assertEqual(triggers.count("      - community/announcements/**"), 2)
        self.assertEqual(triggers.count("      - community/display-translations/**"), 2)
        self.assertIn("format('pr-{0}', github.event.pull_request.number)", self.text)
        self.assertIn("format('push-{0}', github.ref)", self.text)
        self.assertIn("cancel-in-progress: ${{ github.event_name == 'pull_request' || github.event_name == 'push' }}", self.text)

    def test_all_three_preflights_are_independent(self):
        self.assertIn("name: 等待闲置 runner\n", self.jobs["zh-dev-priority-gate"])
        for name in PREFLIGHT_JOBS:
            block = self.jobs[name]
            with self.subTest(job=name):
                self.assertNotRegex(block, r"(?m)^    needs:")
                self.assertNotIn("continue-on-error", block)
                for event, ref in (("push", "refs/heads/zh-dev"),
                                   ("pull_request", "refs/pull/123/merge")):
                    self.assertTrue(job_runs(block, {}, event, ref))

    def test_all_heavy_roots_require_every_preflight_to_succeed(self):
        results = dict.fromkeys(PREFLIGHT_JOBS, "success")
        for name in HEAVY_ROOT_JOBS:
            block = self.jobs[name]
            with self.subTest(job=name):
                self.assertEqual(dependencies(block), PREFLIGHT_JOBS)
                self.assertTrue(job_runs(block, results, "push", "refs/heads/zh-dev"))
            for dep in PREFLIGHT_JOBS:
                for outcome in ("failure", "cancelled", "skipped"):
                    with self.subTest(job=name, preflight=dep, outcome=outcome):
                        self.assertFalse(job_runs(block, results | {dep: outcome}, "push", "refs/heads/zh-dev"))

    def test_heavy_preflight_failures_do_not_start_downstream_artifact_consumers(self):
        aggregates = {"windows-gnu-validation", "windows-gnu-preview", "multiplatform-result"}
        for event, ref in (("push", "refs/heads/zh-dev"),
                           ("pull_request", "refs/pull/123/merge")):
            for failed_preflight in PREFLIGHT_JOBS:
                for outcome in ("failure", "cancelled", "skipped"):
                    results = dict.fromkeys(PREFLIGHT_JOBS, "success") | {failed_preflight: outcome}
                    for name, block in self.jobs.items():
                        if name in PREFLIGHT_JOBS:
                            continue
                        with self.subTest(event=event, job=name, preflight=failed_preflight, outcome=outcome):
                            runs = job_runs(block, results, event, ref)
                            self.assertEqual(runs, name in aggregates)
                            results[name] = "failure" if runs else "skipped"

    def test_sync_push_runs_only_localization_and_never_failing_aggregates(self):
        for outcome in ("success", "failure", "cancelled"):
            with self.subTest(localization=outcome):
                results = {}
                for name, block in self.jobs.items():
                    runs = job_runs(block, results, "push", "refs/heads/sync/upstream-1.0.104")
                    self.assertEqual(runs, name == "localization", name)
                    results[name] = outcome if name == "localization" else "skipped"

    def test_normal_push_pr_schedule_and_manual_build_gates_still_run(self):
        results = dict.fromkeys(self.jobs, "success")
        cases = (("push", "refs/heads/zh-dev"),
                 ("push", "refs/heads/codex/free-account-localization-macos-smoke"),
                 ("pull_request", "refs/pull/123/merge"),
                 ("schedule", "refs/heads/zh-dev"),
                 ("workflow_dispatch", "refs/heads/zh-dev"),
                 ("workflow_dispatch", "refs/heads/sync/upstream-1.0.104"))
        for event, ref in cases:
            for name, block in self.jobs.items():
                if name == "macos-consumer-smoke":
                    continue  # This intentionally has its existing special-branch condition.
                with self.subTest(event=event, ref=ref, job=name):
                    self.assertTrue(job_runs(block, results, event, ref))

    def test_every_build_and_test_is_downstream_of_all_three_preflights(self):
        def ancestors(job):
            block = self.jobs[job]
            required = dependencies(block) if re.search(r"^    needs:", block, re.M) else set()
            return required | {ancestor for dep in required for ancestor in ancestors(dep)}

        for name in self.jobs:
            if name not in PREFLIGHT_JOBS:
                with self.subTest(job=name):
                    self.assertTrue(PREFLIGHT_JOBS.issubset(ancestors(name)))

    def test_aggregate_explicitly_checks_all_preflights_and_rejects_every_non_success(self):
        aggregate = self.jobs["multiplatform-result"]
        self.assertTrue(PREFLIGHT_JOBS.issubset(dependencies(aggregate)))
        result_vars = dict(re.findall(r"^      (\w+): \$\{\{ needs\.([\w-]+)\.result \}\}$", aggregate, re.M))
        self.assertEqual(set(result_vars.values()), dependencies(aggregate))
        artifact_vars = re.findall(r"^      (\w+_ARTIFACT):", aggregate, re.M)
        script = textwrap.dedent(aggregate.split("        run: |\n", 1)[1])
        with tempfile.TemporaryDirectory() as directory:
            env = os.environ | dict.fromkeys(result_vars, "success") | dict.fromkeys(artifact_vars, "artifact")
            env["GITHUB_STEP_SUMMARY"] = str(Path(directory) / "summary")
            self.assertEqual(subprocess.run(["bash", "-c", script], env=env, capture_output=True).returncode, 0)
            for variable in result_vars:
                for outcome in ("failure", "cancelled", "skipped"):
                    with self.subTest(job=result_vars[variable], result=outcome):
                        result = subprocess.run(["bash", "-c", script], env=env | {variable: outcome}, capture_output=True)
                        self.assertNotEqual(result.returncode, 0)

    def test_six_packages_retain_90_days_and_windows_symbols_retain_30(self):
        windows = upload_steps(self.jobs["windows-gnu-build"])
        self.assertEqual(len(windows), 2)
        for upload in windows:
            self.assertRegex(upload, r"name: grok-zh-windows(?:-symbols)?-\$\{\{ github.run_id \}\}-\$\{\{ github.run_attempt \}\}")
            days = 30 if "name: grok-zh-windows-symbols-" in upload else 90
            self.assertIn(f"retention-days: {days}", upload)
        for job, action in PACKAGE_JOBS.items():
            with self.subTest(job=job):
                block = self.jobs[job]
                self.assertIn(f"uses: ./.github/actions/{action}", block)
                self.assertIn("phase: build", block)
                self.assertIn("artifact-retention-days: '90'", block)
        self.assertEqual(self.text.count("artifact-retention-days: '90'"), 5)

    def test_shared_actions_keep_release_package_defaults_and_diagnostic_durations(self):
        release = (ROOT / ".github/workflows/zh-release-windows.yml").read_text(encoding="utf-8")
        self.assertNotIn("artifact-retention-days:", release)
        for name in set(PACKAGE_JOBS.values()):
            action = action_text(name)
            with self.subTest(action=name):
                self.assertRegex(action, r"(?s)  artifact-retention-days:\n.*?    required: false\n    default: '14'\n")
                uploads = upload_steps(action)
                self.assertEqual(len(uploads), 3)  # Product, original Release diagnostics, final preview diagnostics.
                self.assertEqual(sum("retention-days: ${{ inputs.artifact-retention-days }}" in step for step in uploads), 1)
                self.assertEqual(sum("retention-days: 7" in step for step in uploads), 2)
        for name in DIAGNOSTIC_ACTIONS:
            expected = 14 if name == "validate-windows-gnu" else 7
            for upload in diagnostic_steps(action_text(name)):
                with self.subTest(action=name):
                    self.assertIn(f"retention-days: {expected}", upload)

    def test_successful_preview_events_and_cancellation_do_not_upload_diagnostics(self):
        for name in DIAGNOSTIC_ACTIONS:
            diagnostics = diagnostic_steps(action_text(name))
            self.assertTrue(diagnostics, name)
            for event in ("push", "pull_request", "schedule", "workflow_dispatch"):
                for phase in ("all", "build", "test"):
                    for status in ("success", "cancelled", "skipped"):
                        with self.subTest(action=name, event=event, phase=phase, status=status):
                            self.assertFalse(any(step_runs(step, status, phase=phase, event=event)
                                                 for step in diagnostics))
                            # A failure followed by cancellation must not create an upload either.
                            self.assertFalse(any(step_runs(step, "cancelled", phase=phase, event=event,
                                                           previous_failure=True) for step in diagnostics))

    def test_preview_failure_handlers_are_last_and_cover_late_failures(self):
        for name in DIAGNOSTIC_ACTIONS:
            steps = step_blocks(action_text(name))
            diagnostics = diagnostic_steps(action_text(name))
            for phase in ("all", "build", "test"):
                expected = 0 if phase == "test" and name in ("build-linux-x64", "build-macos-arm") else 1
                handlers = [step for step in diagnostics if step_runs(step, "failure", phase=phase)]
                with self.subTest(action=name, phase=phase):
                    self.assertEqual(len(handlers), expected)
                if not handlers:
                    continue
                handler = handlers[0]
                self.assertEqual(handler, steps[-1], name)
                self.assertIn("failure() && !cancelled()", handler)
                # Inject a failure at every preceding step, including product upload,
                # final validation/summary, and cache-save steps after the old upload.
                for failure_index, failed_step in enumerate(steps[:-1]):
                    with self.subTest(action=name, phase=phase, failed=failed_step.splitlines()[0]):
                        failed = False
                        uploaded = []
                        for index, step in enumerate(steps):
                            if index == failure_index:
                                failed = True
                            if step in diagnostics and step_runs(step, "failure" if failed else "success", phase=phase):
                                uploaded.append(step)
                        self.assertEqual(uploaded, [handler])

    def test_resource_monitor_timings_and_validation_finish_before_final_evidence(self):
        producer_markers = {
            "build-windows-gnu-cross": ("/usr/bin/time -v", "--timings", "build-input.json"),
            "build-windows-msvc-cross": ("--timings", "msvc-build-input.py write"),
            "build-windows-arm": ("--timings", "package-windows-arm.ps1"),
            "build-windows-msvc-x64": ("--timings", "package-windows-msvc-x64.ps1"),
            "build-linux-x64": ("--features release-dist --timings", "strip-unix-binary.py", "flock -n 9"),
            "build-macos-arm": ("macos-build.py build", "strip-unix-binary.py", "verify-macos-package.sh"),
            "validate-windows-gnu": ("run-windows-validation.ps1",),
        }
        for name, markers in producer_markers.items():
            text = action_text(name)
            final = diagnostic_steps(text)[-1]
            final_offset = text.index(final)
            for marker in markers:
                with self.subTest(action=name, producer=marker):
                    self.assertLess(text.index(marker), final_offset)
                    self.assertTrue(step_runs(final, "failure"))
        validation = action_text("validate-windows-gnu")
        final = diagnostic_steps(validation)[-1]
        self.assertIn("grok-zh-target/cargo-timings", final)
        self.assertIn("grok-zh-windows-validation-${{ inputs.suite }}", final)
        # Resource finalization is in the validation command's finally block.
        runner = (ROOT / ".github/scripts/run-windows-validation.ps1").read_text(encoding="utf-8")
        self.assertIn("} finally {", runner)
        self.assertIn("'summary.json'", runner)
        self.assertIn("-Mode Summarize", runner)
        self.assertIn("'resources.json'", runner)

    def test_release_diagnostic_gates_keep_original_status_and_phase_behavior(self):
        for name in ("build-linux-x64", "build-macos-arm", "build-windows-arm",
                     "build-windows-msvc-x64", "validate-windows-gnu"):
            steps = diagnostic_steps(action_text(name))
            self.assertEqual(len(steps), 2)
            for phase in ("all", "build", "test"):
                for status in ("success", "failure", "cancelled"):
                    expected = (status != "cancelled" if name in (
                        "build-linux-x64", "build-windows-arm", "build-windows-msvc-x64") else True)
                    if name in ("build-linux-x64", "build-macos-arm"):
                        expected = expected and phase != "test"
                    with self.subTest(action=name, phase=phase, status=status):
                        self.assertEqual(step_runs(steps[0], status, release=True, phase=phase), expected)
                        self.assertFalse(step_runs(steps[1], status, release=True, phase=phase))
            if name == "build-macos-arm":
                self.assertFalse(step_runs(steps[0], release=True, report_dir=""))

    def test_linux_preview_package_excludes_diagnostics_but_release_keeps_them(self):
        text = action_text("build-linux-x64")
        product = [step for step in upload_steps(text) if "id: preview_artifact" in step]
        self.assertEqual(len(product), 1)
        self.assertIn("${{ inputs.release_build == 'true' && 'dist/diagnostics/linux/' || '' }}", product[0])
        self.assertNotRegex(product[0], r"(?m)^ +dist/diagnostics/linux/$")
        self.assertIn("dist/diagnostics/linux/", diagnostic_steps(text)[-1])

    def test_intermediate_inputs_stay_7_days_and_are_not_failure_gated(self):
        for name, payload in (("build-windows-gnu-cross", "grok-gnu-cross-payload"),
                              ("build-windows-msvc-cross", "grok-msvc-cross-input")):
            inputs = [step for step in upload_steps(action_text(name)) if "if-no-files-found: error" in step]
            self.assertEqual(len(inputs), 1)
            self.assertIn(f"path: ${{{{ runner.temp }}}}/{payload}", inputs[0])
            self.assertIn("retention-days: 7", inputs[0])
            self.assertIn("compression-level: 1", inputs[0])
            self.assertTrue(step_runs(inputs[0]))
            self.assertFalse(step_runs(inputs[0], "failure"))
            self.assertFalse(step_runs(inputs[0], "cancelled"))

    def test_msvc_migration_product_and_tests_remain_isolated_and_unchanged(self):
        migration = [step for step in upload_steps(self.jobs["windows-msvc-migration-experiment"])
                     if "name: migration-experiment-grok-zh-" in step]
        self.assertEqual(len(migration), 1)
        self.assertIn("retention-days: 14", migration[0])
        self.assertIn("name: migration-experiment-grok-zh-", migration[0])
        self.assertEqual(self.jobs["windows-msvc-migration-experiment"].count("Test-EmbeddedMigrationProduct.ps1"), 2)
        self.assertEqual(self.jobs["windows-msvc-migration-test"].count("Test-WindowsMigration.ps1"), 2)
        self.assertIn("phase: test", self.jobs["windows-msvc-migration-test"])
        self.assertIn("msvc-build-input.py verify", self.jobs["windows-msvc-migration-experiment"])


if __name__ == "__main__":
    unittest.main()
