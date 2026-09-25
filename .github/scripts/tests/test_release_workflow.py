"""Check the Release publication gates without executing a build or publishing."""

import ast
from itertools import product
from pathlib import Path
import re
import unittest


ROOT = Path(__file__).resolve().parents[3]
WORKFLOW = ROOT / ".github/workflows/zh-release-windows.yml"
VALIDATION = "windows-x64-gnu-validation"
BUILD = "windows-x64-gnu"
RUST = "windows-x64-gnu-rust-validation"


def job_blocks(text):
    # This workflow uses unquoted job IDs indented by two spaces. Reject missing
    # jobs/fields in the tests rather than silently assuming a gate is present.
    return dict(re.findall(r"^  ([\w-]+):\n(.*?)(?=^  [\w-]+:\n|\Z)", text, re.M | re.S))


def dependencies(block):
    field = re.search(r"^    needs:(.*)\n((?:      - [\w-]+\n)*)", block, re.M)
    if field is None:
        raise AssertionError("missing needs field")
    scalar, sequence = field.groups()
    return {scalar.strip()} if scalar.strip() else set(re.findall(r"- ([\w-]+)", sequence))


def gate_allows(block, results, include_unix=True, include_new_platforms=True, cancelled=False):
    field = re.search(r"^    if: >-\n((?:      .*\n)+)", block, re.M)
    if field is None:
        raise AssertionError("missing explicit publication condition")
    expression = " ".join(field[1].replace("${{", "").replace("}}", "").split())
    expression = re.sub(
        r"needs\.([\w-]+)\.result", lambda match: repr(results[match[1]]), expression,
    )
    expression = re.sub(
        r"needs\.release-plan\.outputs\.include_(macos|linux|new_platforms)",
        lambda match: repr(str(include_new_platforms if match[1] == "new_platforms" else include_unix).lower()),
        expression,
    )
    expression = expression.replace("!cancelled()", str(not cancelled))
    expression = expression.replace("&&", " and ").replace("||", " or ")
    tree = ast.parse(expression, mode="eval")
    allowed = (ast.Expression, ast.BoolOp, ast.And, ast.Or, ast.Compare, ast.Eq, ast.Constant)
    if any(not isinstance(node, allowed) for node in ast.walk(tree)):
        raise AssertionError("unsupported gate expression; update the explicit evaluator")
    return eval(compile(tree, str(WORKFLOW), "eval"), {"__builtins__": {}}, {})


class ReleaseWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = WORKFLOW.read_text(encoding="utf-8")
        cls.jobs = job_blocks(cls.text)

    def test_new_platform_release_floor_matches_the_first_six_asset_version(self):
        plan = self.jobs["release-plan"]
        self.assertIn("$patch -ge 36", plan)
        self.assertIn("include_new_platforms = $includeNewPlatforms.ToString().ToLowerInvariant()", plan)

    def test_windows_validation_and_build_run_independently_at_the_same_commit(self):
        for job in (RUST, BUILD):
            with self.subTest(job=job):
                block = self.jobs[job]
                self.assertEqual(dependencies(block), {"release-plan"})
                self.assertIn("ref: ${{ needs.release-plan.outputs.source_commit }}", block)
                self.assertIn("version: ${{ needs.release-plan.outputs.grok_version }}", block)
                self.assertIn("uses: ./.github/actions/setup-windows-gnu", block)
                self.assertIn('CARGO_BUILD_JOBS: "4"', block)
                self.assertNotIn("contents: write", block)
                self.assertNotIn("id-token: write", block)
        self.assertEqual(dependencies(self.jobs[VALIDATION]), {"release-plan", RUST})
        self.assertIn("if: always()", self.jobs[VALIDATION])
        self.assertIn('all(. == "success")', self.jobs[VALIDATION])
        self.assertIn("${{ toJSON(needs.*.result) }}", self.jobs[VALIDATION])
        self.assertIn('CARGO_PROFILE_TEST_DEBUG: "0"', self.jobs[RUST])
        self.assertNotIn("CARGO_PROFILE_TEST_DEBUG", self.jobs[BUILD])
        self.assertNotIn("cargo build ", self.jobs[RUST])
        self.assertNotIn("cargo test ", self.jobs[BUILD])

    def test_publication_requires_every_enabled_platform_and_validation(self):
        for job in ("release-attestations", "release-publisher"):
            block = self.jobs[job]
            required = dependencies(block)
            self.assertTrue({
                "release-plan", VALIDATION, BUILD, "windows-arm64-msvc-release",
                "macos-arm-release", "macos-intel-release", "linux-x64-gnu-release",
                "linux-arm64-gnu-release",
            }.issubset(required))
            baseline = dict.fromkeys(required, "success")
            self.assertTrue(gate_allows(block, baseline))
            self.assertFalse(gate_allows(block, baseline, cancelled=True))
            for failed_job, outcome in product(required, ("failure", "cancelled", "skipped")):
                with self.subTest(gate=job, failed_job=failed_job, outcome=outcome):
                    self.assertFalse(gate_allows(block, baseline | {failed_job: outcome}))

    def test_legacy_bridge_only_allows_the_explicitly_disabled_platform_jobs_to_skip(self):
        for job in ("release-attestations", "release-publisher"):
            block = self.jobs[job]
            results = dict.fromkeys(dependencies(block), "success")
            results.update({
                "macos-arm-release": "skipped", "linux-x64-gnu-release": "skipped",
                "windows-arm64-msvc-release": "skipped", "macos-intel-release": "skipped",
                "linux-arm64-gnu-release": "skipped",
            })
            self.assertTrue(gate_allows(block, results, include_unix=False, include_new_platforms=False))
            for failed_job in (VALIDATION, BUILD):
                for outcome in ("failure", "cancelled", "skipped"):
                    with self.subTest(gate=job, failed_job=failed_job, outcome=outcome):
                        self.assertFalse(gate_allows(
                            block, results | {failed_job: outcome}, include_unix=False,
                            include_new_platforms=False,
                        ))

    def test_all_six_assets_are_downloaded_attested_and_published(self):
        attest = self.jobs["release-attestations"]
        publish = self.jobs["release-publisher"]
        for job in ("windows-arm64-msvc-release", "macos-intel-release", "linux-arm64-gnu-release"):
            self.assertIn(f"needs.{job}.outputs.artifact_name", attest)
            self.assertIn(f"needs.{job}.outputs.artifact_name", publish)
            self.assertIn(f"needs.{job}.outputs.archive_name", attest)
            self.assertIn(f"needs.{job}.outputs.archive_name", publish)
        for name in ("WINDOWS_ARM_ARCHIVE_NAME", "MAC_INTEL_ARCHIVE_NAME", "LINUX_ARM_ARCHIVE_NAME"):
            self.assertIn(f"env.{name}", attest)
            self.assertIn(f"$env:{name}", publish)

    def test_formal_macos_keeps_release_identity_and_publisher_owns_release_writes(self):
        macos = self.jobs["macos-arm-release"]
        self.assertIn("release_build: 'true'", macos)
        self.assertIn("version: ${{ needs.release-plan.outputs.grok_version }}", macos)
        self.assertIn("ref: ${{ needs.release-plan.outputs.source_commit }}", macos)
        self.assertEqual(
            [job for job, block in self.jobs.items() if "contents: write" in block],
            ["release-publisher"],
        )
        self.assertIn("release-attestations", dependencies(self.jobs["release-publisher"]))


class PreviewNativeValidationTests(unittest.TestCase):
    def test_all_native_test_jobs_gate_the_six_artifacts(self):
        jobs = job_blocks((ROOT / ".github/workflows/zh-dev-windows-preview.yml").read_text(encoding="utf-8"))
        tests = jobs["native-rust-validation"]
        self.assertNotIn("needs:", tests)  # Tests start alongside production builds.
        self.assertIn("fail-fast: false", tests)
        targets = re.findall(r"^            target: (\S+)$", tests, re.M)
        self.assertCountEqual(targets, [
            "aarch64-pc-windows-msvc", "x86_64-unknown-linux-gnu",
            "aarch64-unknown-linux-gnu", "aarch64-apple-darwin", "x86_64-apple-darwin",
        ])
        self.assertEqual(tests.count("phase: test"), 3)
        gate = jobs["multiplatform-result"]
        self.assertIn("native-rust-validation", dependencies(gate))
        self.assertIn("NATIVE_TEST_RESULT: ${{ needs.native-rust-validation.result }}", gate)
        self.assertIn('"${NATIVE_TEST_RESULT}" != success ||', gate)
        self.assertIn("native-rust-validation", dependencies(jobs["macos-consumer-smoke"]))
        for job in ("windows-arm64-msvc-preview", "linux-x64-gnu-preview",
                    "linux-arm64-gnu-preview", "macos-arm-preview", "macos-intel-preview"):
            with self.subTest(job=job):
                self.assertNotIn("needs:", jobs[job])
                self.assertIn("phase: build", jobs[job])
                self.assertIn(job, dependencies(gate))

    def test_intel_cross_build_requires_native_artifact_and_test_validation(self):
        jobs = job_blocks((ROOT / ".github/workflows/zh-dev-windows-preview.yml").read_text(encoding="utf-8"))
        build = jobs["macos-intel-preview"]
        self.assertIn("runs-on: macos-15\n", build)
        self.assertIn("RUSTUP_TOOLCHAIN: 1.94.0-aarch64-apple-darwin", build)
        self.assertIn("TARGET: x86_64-apple-darwin", build)
        self.assertIn("cross_compile: 'true'", build)
        native = jobs["macos-intel-native-smoke"]
        self.assertEqual(dependencies(native), {"macos-intel-preview"})
        self.assertIn("runs-on: macos-15-intel", native)
        self.assertIn("MACOS_VERIFY_MODE: native", native)
        self.assertIn("needs.macos-intel-preview.outputs.artifact_name", native)
        self.assertIn("verify-macos-package.sh", native)
        self.assertNotIn("continue-on-error", native)
        gate = jobs["multiplatform-result"]
        self.assertIn("macos-intel-native-smoke", dependencies(gate))
        self.assertIn("MACOS_INTEL_NATIVE_RESULT: ${{ needs.macos-intel-native-smoke.result }}", gate)
        self.assertIn('"${MACOS_INTEL_NATIVE_RESULT}" != success', gate)
        self.assertRegex(jobs["native-rust-validation"],
                         r"(?s)label: macOS Intel\n\s+os: macos-15-intel\n.*?target: x86_64-apple-darwin")

    def test_formal_release_keeps_full_validation_and_every_preview_phase_fetches(self):
        release_jobs = job_blocks(WORKFLOW.read_text(encoding="utf-8"))
        for name in ("build-windows-arm", "build-linux-x64", "build-macos-arm"):
            with self.subTest(action=name):
                action = (ROOT / f".github/actions/{name}/action.yml").read_text(encoding="utf-8")
                self.assertRegex(action, r"(?s)  phase:.*?    default: all")
                self.assertIn("正式 Release 必须执行完整测试与构建", action)
                steps = re.split(r"^\s+- name: ", action, flags=re.M)[1:]
                fetches = [s for s in steps if "cargo fetch --locked" in s]
                self.assertEqual(len(fetches), 1)
                self.assertNotIn("if:", fetches[0])
                tests = [s for s in steps if "cargo test --frozen" in s]
                self.assertEqual(len(tests), 1)
                self.assertIn("if: inputs.phase != 'build'", tests[0])
                self.assertNotIn("cache-hit", tests[0])
                for job, block in release_jobs.items():
                    if f"uses: ./.github/actions/{name}" in block:
                        self.assertNotIn("phase:", block, job)
                        self.assertIn("release_build: 'true'", block, job)


if __name__ == "__main__":
    unittest.main()
