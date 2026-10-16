#!/usr/bin/env python3
"""Tests for the harness, not substitutes for real Windows binary execution."""
import importlib.util
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import MagicMock, patch
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('direct', ROOT / '.github/scripts/windows-direct-replacement.py')
direct = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(direct)


class TestPinnedInputs(unittest.TestCase):
    def test_only_separate_test_branch_can_execute(self):
        text = (ROOT / '.github/workflows/zh-dev-windows-preview.yml').read_text(encoding='utf-8')
        block = text.split('  windows-direct-replacement:\n')[1]
        self.assertIn("github.ref == 'refs/heads/test/windows-msvc-direct-replacement'", block)
        self.assertIn("github.event_name == 'workflow_dispatch'", block)
        self.assertIn('inputs.direct_replacement_only', block)
        self.assertNotIn('contents: write', block)
        self.assertNotIn('actions: write', block)
        self.assertNotIn('uses: ./.github/actions/build-', block)

    def test_every_other_job_is_gated(self):
        import re
        text = (ROOT / '.github/workflows/zh-dev-windows-preview.yml').read_text(encoding='utf-8').split('jobs:\n',1)[1]
        blocks = re.split(r'^  ([a-z0-9-]+):\n', text, flags=re.M)
        for name, block in zip(blocks[1::2], blocks[2::2]):
            if name != 'windows-direct-replacement':
                self.assertIn("!(inputs.direct_replacement_only || (github.event_name == 'push' && github.ref == 'refs/heads/test/windows-msvc-direct-replacement'))", block, name)

    def test_production_switch_still_disabled(self):
        policy = json.loads((ROOT / '.github/release-policy.json').read_text(encoding='utf-8'))
        self.assertFalse(policy['windows_x64_msvc']['enabled'])

    def test_pinned_real_binaries_not_fixture_launchers(self):
        inputs = json.loads(direct.INPUTS.read_text(encoding='utf-8'))
        self.assertEqual(inputs['historical']['source_commit'], '3f5d07f8cb2e9be395131296a3efc4635713db7f')
        self.assertEqual(inputs['native']['run_id'], 38842814822)
        self.assertEqual(inputs['native']['artifact_id'], 12022879975)
        self.assertEqual(inputs['native']['executable_sha256'], '8626e60c908f44848190adc9f3547241c651ed8179601d93e33b91469dc76f20')
        self.assertGreater(inputs['historical']['archive_bytes'], 50_000_000)
        self.assertGreater(inputs['native']['archive_bytes'], 50_000_000)

    def test_no_migration_or_installer_execution(self):
        text = (ROOT / '.github/scripts/windows-direct-replacement.py').read_text(encoding='utf-8')
        self.assertNotIn('powershell.exe', text)
        self.assertNotIn('Invoke-Bootstrap', text)
        self.assertNotIn('ExecutionPolicy', text)
        self.assertIn("changed == ['grok-zh.exe']", text)
        self.assertIn('retained_metadata_is_stale_by_design', text)

    def test_output_upload_excludes_binary_archives(self):
        text = (ROOT / '.github/workflows/zh-dev-windows-preview.yml').read_text(encoding='utf-8').split('  windows-direct-replacement:\n')[1]
        paths = text.split('          path: |\n')[1].split('          if-no-files-found')[0]
        self.assertNotIn('*.zip', paths)
        self.assertNotIn('*.exe', paths)
        self.assertNotIn('direct-results/**', paths)


class TestArchiveSafety(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
    def tearDown(self):
        self.tmp.cleanup()
    def make(self, name, data=b'content'):
        archive = self.root / 'test.zip'
        with zipfile.ZipFile(archive, 'w') as z:
            if isinstance(name, str):
                # Preserve deliberately malformed names on every platform;
                # ZipInfo's constructor normalizes Windows backslashes.
                entry = zipfile.ZipInfo(name)
                entry.filename = name
            else:
                entry = name
            z.writestr(entry, data)
        return archive
    def test_extract_regular_file(self):
        direct.extract_zip(self.make('package/file'), self.root/'out')
        self.assertEqual((self.root/'out/package/file').read_bytes(), b'content')
    def test_reject_parent_path(self):
        with self.assertRaises(ValueError): direct.extract_zip(self.make('../bad'), self.root/'out')
    def test_reject_absolute_path(self):
        with self.assertRaises(ValueError): direct.extract_zip(self.make('/bad'), self.root/'out')
    def test_reject_windows_path(self):
        with self.assertRaises(ValueError): direct.extract_zip(self.make('C:/bad'), self.root/'out')
    def test_reject_backslash(self):
        with self.assertRaises(ValueError): direct.extract_zip(self.make('x\\bad'), self.root/'out')
    def test_reject_original_name_after_windows_normalization(self):
        archive = self.make('x\\bad')
        with patch.object(zipfile.os, 'sep', '\\'):
            with self.assertRaisesRegex(ValueError, 'Normalized ZIP path'):
                direct.extract_zip(archive, self.root/'out')
    def test_reject_original_name_with_nul(self):
        with self.assertRaisesRegex(ValueError, 'Normalized ZIP path'):
            direct.extract_zip(self.make('x\0bad'), self.root/'out')
    def test_reject_trailing_windows_dot(self):
        with self.assertRaises(ValueError): direct.extract_zip(self.make('x./bad'), self.root/'out')
    def test_reject_symlink(self):
        i = zipfile.ZipInfo('link'); i.external_attr = 0o120777 << 16
        with self.assertRaises(ValueError): direct.extract_zip(self.make(i), self.root/'out')
    def test_reject_case_duplicate(self):
        archive=self.make('A')
        with zipfile.ZipFile(archive,'a') as z:z.writestr('a',b'other')
        with self.assertRaises(ValueError): direct.extract_zip(archive, self.root/'out')
    def test_reject_existing_destination(self):
        out=self.root/'out';out.mkdir()
        with self.assertRaises(ValueError): direct.extract_zip(self.make('file'),out)
    def test_hash_mismatch_rejected(self):
        p=self.root/'file';p.write_bytes(b'not the expected binary')
        with self.assertRaises(ValueError): direct.verify_file(p,'0'*64)
    def test_size_mismatch_rejected(self):
        p=self.root/'file';p.write_bytes(b'test')
        with self.assertRaises(ValueError): direct.verify_file(p,direct.digest(p),99)
    def test_pe_rejects_non_executable(self):
        p=self.root/'file';p.write_bytes(b'x'*256)
        with self.assertRaises(ValueError): direct.inspect_pe(p)
    def test_safe_environment_drops_tokens(self):
        previous=dict(os.environ)
        try:
            os.environ['SystemRoot']='C:/Windows';os.environ['GH_TOKEN']='test-secret'
            env=direct.isolated_environment(self.root/'user')
            self.assertNotIn('GH_TOKEN',env)
            self.assertNotIn('GITHUB_TOKEN',env)
            self.assertEqual(env['GROK_HOME'],str(self.root/'user/home/.grok-direct-test'))
        finally:
            os.environ.clear();os.environ.update(previous)



class TestBoundedCommands(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.process = MagicMock()
        self.process.pid = 123
        self.process.returncode = 0
        self.env = {'SystemRoot': 'C:/Windows'}
        self.stdout = b'grok-zh 1.0.103 (real)\n'
    def tearDown(self):
        self.tmp.cleanup()
    def launch(self, *args, **kwargs):
        kwargs['stdout'].write(self.stdout)
        return self.process
    def run_command(self):
        return direct.run_command(['grok-zh.exe', '--version'], self.root, self.env, 'test', self.root, '1.0.103')
    def test_success_records_version_and_logs(self):
        with patch.object(direct.subprocess, 'Popen', side_effect=self.launch):
            result=self.run_command()
        self.assertEqual(result['returncode'],0)
        self.assertEqual((self.root/'test.stdout.txt').read_bytes(),self.stdout)
    def test_wrong_version_is_failure(self):
        self.stdout = b'grok-zh 1.0.45 (old)\n'
        with patch.object(direct.subprocess, 'Popen', side_effect=self.launch):
            with self.assertRaisesRegex(ValueError,'unexpected product version'):self.run_command()
    def timeout_case(self, cleanup_side_effect=None, second_timeout=False):
        error=subprocess.TimeoutExpired('grok-zh',35)
        self.stdout=b'partial'
        self.process.wait.side_effect=[error, error if second_timeout else 0]
        self.process.poll.side_effect=[None,None if second_timeout else 0]
        cleanup=MagicMock(returncode=1,stdout=b'',stderr=b'cleanup failure')
        with patch.object(direct.subprocess,'Popen',side_effect=self.launch), patch.object(direct.subprocess,'run',return_value=cleanup,side_effect=cleanup_side_effect):
            with self.assertRaisesRegex(ValueError,'exceeded 35 seconds'):self.run_command()
        self.process.kill.assert_called_once()
        record=json.loads((self.root/'test.cleanup.txt').read_text(encoding='utf-8'))
        self.assertEqual(record['root_termination_confirmed'],not second_timeout)
        self.assertEqual((self.root/'test.stdout.txt').read_bytes(),b'partial')
        return record
    def test_taskkill_timeout_still_kills_and_logs(self):
        record=self.timeout_case(subprocess.TimeoutExpired('taskkill',10))
        self.assertIsNotNone(record['error'])
    def test_taskkill_launch_failure_still_kills_and_logs(self):
        record=self.timeout_case(OSError('taskkill unavailable'))
        self.assertIn('unavailable',record['error'])
    def test_nonzero_taskkill_still_kills_and_logs(self):
        record=self.timeout_case()
        self.assertEqual(record['returncode'],1)
    def test_second_wait_timeout_is_bounded(self):
        record=self.timeout_case(second_timeout=True)
        self.assertEqual(record['root_wait'],'timed_out')


if __name__ == '__main__':
    unittest.main()
