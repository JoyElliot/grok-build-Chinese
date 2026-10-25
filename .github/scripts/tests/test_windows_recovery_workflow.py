"""Guard the isolated recovery CI against accidental release/publication expansion."""
import pathlib
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[3]


class RecoveryWorkflowTests(unittest.TestCase):
    def test_isolated_branch_and_readonly_permissions(self):
        workflow = (ROOT / '.github/workflows/zh-windows-recovery-regression.yml').read_text()
        self.assertIn("if: github.ref == 'refs/heads/test/windows-recovery-risks'", workflow)
        self.assertIn('contents: read\n  actions: read', workflow)
        self.assertNotIn('contents: write', workflow)
        self.assertNotIn('gh release', workflow)
        self.assertNotIn('git push', workflow)
        self.assertNotIn('workflow_run:', workflow)
        self.assertIn('direct-transaction-validation/Cargo.toml', workflow)
        self.assertIn("CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS: 'true'", workflow)
        self.assertIn("CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS: 'true'", workflow)
        self.assertIn('WindowsPowerShell/v1.0/powershell.exe', workflow)
        self.assertIn('transaction-acceptance/prepare.py download', workflow)


if __name__ == '__main__':
    unittest.main()
