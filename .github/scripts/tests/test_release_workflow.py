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


def gate_allows(block, results, include_unix=True, cancelled=False):
    field = re.search(r"^    if: >-\n((?:      .*\n)+)", block, re.M)
    if field is None:
        raise AssertionError("missing explicit publication condition")
    expression = " ".join(field[1].replace("${{", "").replace("}}", "").split())
    expression = re.sub(
        r"needs\.([\w-]+)\.result", lambda match: repr(results[match[1]]), expression,
    )
    expression = re.sub(
        r"needs\.release-plan\.outputs\.include_(?:macos|linux)",
        repr(str(include_unix).lower()), expression,
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

    def test_windows_validation_and_build_run_independently_at_the_same_commit(self):
        for job in (VALIDATION, BUILD):
            with self.subTest(job=job):
                block = self.jobs[job]
                self.assertEqual(dependencies(block), {"release-plan"})
                self.assertIn("ref: ${{ needs.release-plan.outputs.source_commit }}", block)
                self.assertIn("version: ${{ needs.release-plan.outputs.grok_version }}", block)
                self.assertIn("uses: ./.github/actions/setup-windows-gnu", block)
                self.assertIn('CARGO_BUILD_JOBS: "4"', block)
                self.assertNotIn("contents: write", block)
                self.assertNotIn("id-token: write", block)
        self.assertIn('CARGO_PROFILE_TEST_DEBUG: "0"', self.jobs[VALIDATION])
        self.assertNotIn("CARGO_PROFILE_TEST_DEBUG", self.jobs[BUILD])
        self.assertNotIn("cargo build ", self.jobs[VALIDATION])
        self.assertNotIn("cargo test ", self.jobs[BUILD])

    def test_publication_requires_every_enabled_platform_and_validation(self):
        for job in ("release-attestations", "release-publisher"):
            block = self.jobs[job]
            required = dependencies(block)
            self.assertTrue({"release-plan", VALIDATION, BUILD}.issubset(required))
            baseline = dict.fromkeys(required, "success")
            self.assertTrue(gate_allows(block, baseline))
            self.assertFalse(gate_allows(block, baseline, cancelled=True))
            for failed_job, outcome in product(required, ("failure", "cancelled", "skipped")):
                with self.subTest(gate=job, failed_job=failed_job, outcome=outcome):
                    self.assertFalse(gate_allows(block, baseline | {failed_job: outcome}))

    def test_legacy_bridge_only_allows_the_explicitly_disabled_unix_jobs_to_skip(self):
        for job in ("release-attestations", "release-publisher"):
            block = self.jobs[job]
            results = dict.fromkeys(dependencies(block), "success")
            results.update({"macos-arm-release": "skipped", "linux-x64-gnu-release": "skipped"})
            self.assertTrue(gate_allows(block, results, include_unix=False))
            for failed_job in (VALIDATION, BUILD):
                for outcome in ("failure", "cancelled", "skipped"):
                    with self.subTest(gate=job, failed_job=failed_job, outcome=outcome):
                        self.assertFalse(gate_allows(
                            block, results | {failed_job: outcome}, include_unix=False,
                        ))

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


if __name__ == "__main__":
    unittest.main()
