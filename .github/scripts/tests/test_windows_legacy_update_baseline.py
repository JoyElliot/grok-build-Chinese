#!/usr/bin/env python3
"""Accounting tests only; no binary launch or network update occurs here."""
import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('baseline', ROOT / '.github/scripts/windows-legacy-update-baseline.py')
baseline = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(baseline)


class TestActualReplacementEvidence(unittest.TestCase):
    def setUp(self):
        self.before = {'grok-zh.exe': 'oldhash', 'rg.exe': 'rghash'}
        self.after = {**self.before, 'grok-zh.exe.old': 'oldhash', 'grok-zh.exe.update.lock': 'lockhash'}
        self.first = {'volume': 1, 'file_index': '10'}
        self.second = {'volume': 1, 'file_index': '11'}
        self.stderr = '已从 JoyElliot/grok-build-Chinese/releases 安装 grok-zh v1.0.45。'

    def check(self):
        return baseline.verify_reinstall_evidence(self.before, self.after, self.first, self.second, 'oldhash', self.stderr)

    def test_same_hash_with_new_file_identity_and_backup_is_required(self):
        self.assertEqual(self.check(), ['grok-zh.exe.old'])

    def test_no_op_same_hash_is_not_pass(self):
        self.second = self.first
        with self.assertRaisesRegex(ValueError, 'No actual EXE'):
            self.check()

    def test_absent_transaction_log_is_not_pass(self):
        self.stderr = 'already latest'
        with self.assertRaisesRegex(ValueError, 'completion output'):
            self.check()

    def test_changed_payload_or_companion_is_rejected(self):
        for name in ['grok-zh.exe', 'rg.exe']:
            self.after[name] = 'changed'
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, 'original package content'):
                self.check()
            self.after[name] = self.before[name]

    def test_backup_identity_and_unexpected_files_rejected(self):
        self.after['grok-zh.exe.old'] = 'changed'
        with self.assertRaisesRegex(ValueError, 'backup differs'):
            self.check()
        self.after['grok-zh.exe.old'] = 'oldhash'
        self.after['candidate.exe'] = 'oldhash'
        with self.assertRaisesRegex(ValueError, 'Unexpected baseline package files'):
            self.check()

    def test_scoped_fixed_version_invocation(self):
        text = (ROOT / '.github/scripts/windows-legacy-update-baseline.py').read_text(encoding='utf-8')
        self.assertIn("'update', '--version', old['version'], '--force-reinstall'", text)
        self.assertIn('timeout_seconds=300', text)
        self.assertIn("'msvc_compatibility_proven': False", text)
        self.assertIn('shutil.copytree(source, package)', text)
        self.assertNotIn('fetch_json(', text)


if __name__ == '__main__':
    unittest.main()
