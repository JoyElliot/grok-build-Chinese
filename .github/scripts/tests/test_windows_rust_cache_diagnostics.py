"""Reject private log payloads and bound optional cache evidence."""

import contextlib
import importlib.util
import io
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock


ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location(
    "cache_diagnostics", ROOT / ".github/scripts/windows-rust-cache-diagnostics.py")
DIAG = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(DIAG)


class CacheDiagnosticsTests(unittest.TestCase):
    def test_only_complete_known_key_lines_survive(self):
        key = "a" * 64
        valid = f"[2026-09-22T05:41:49Z DEBUG sccache::compiler::compiler] [grok_lib]: Hash key: {key}"
        raw = "\n".join((valid, 'TOKEN=private', valid + ' TOKEN=private',
                          valid.replace('grok_lib', 'C:/private/path'),
                          valid.replace('DEBUG', 'TRACE'),
                          valid.replace(key, key + 'a'), 'compiler error: secret'))
        self.assertEqual(DIAG.request_keys(raw.encode()), [{"crate": "grok_lib", "key": key}])

    def test_log_and_request_limits_fail_closed(self):
        with mock.patch.object(DIAG, 'MAX_LOG_BYTES', 2):
            with self.assertRaises(ValueError):
                DIAG.request_keys(b'xxx')
        line = f"[2026-09-22T00:00:00Z DEBUG sccache::compiler::compiler] [lib]: Hash key: {'a' * 64}\n"
        with mock.patch.object(DIAG, 'MAX_KEYS', 1):
            with self.assertRaises(ValueError):
                DIAG.request_keys((line * 2).encode())

    def test_disk_names_require_full_key_and_correct_directories(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'a/b').mkdir(parents=True)
            key = 'ab' + 'c' * 62
            (root / 'a/b' / key).write_bytes(b'cached')
            (root / 'a/b' / ('f' * 64)).write_bytes(b'wrong directory')
            (root / 'a/b/private-token').write_bytes(b'private')
            self.assertEqual(DIAG.disk_keys(root), {key: 6})

    def test_summarize_matches_precompile_snapshot_not_new_writes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            report, private = root / 'report.json', root / 'private.log'
            old, new = 'a' * 64, 'b' * 64
            report.write_text(json.dumps({'restored_keys': {old: 10}, 'unknown': 'private'}))
            private.write_text('\n'.join(
                f'[2026-09-22T00:00:00Z DEBUG sccache::compiler::compiler] [lib]: Hash key: {k}'
                for k in (old, new)))
            env = {'GROK_SCCACHE_ACTIVE': 'true', 'SCCACHE_DIR': directory}
            with mock.patch.dict(os.environ, env), mock.patch.object(DIAG, 'paths', return_value=(report, private)), contextlib.redirect_stdout(io.StringIO()):
                DIAG.summarize()
            data = json.loads(report.read_text())
            self.assertEqual(data['matching_restored_requests'], 1)
            self.assertEqual([r['present_before_compile'] for r in data['requests']], [True, False])
            self.assertNotIn('unknown', data)

    def test_private_log_isolated_by_run_and_attempt(self):
        env = {'RUNNER_TEMP': 'temp', 'CARGO_TARGET_DIR': 'target',
               'GITHUB_RUN_ID': '123', 'GITHUB_RUN_ATTEMPT': '1'}
        with mock.patch.dict(os.environ, env):
            first = DIAG.paths()
            os.environ['GITHUB_RUN_ATTEMPT'] = '2'
            self.assertNotEqual(first[0], DIAG.paths()[0])
            self.assertNotEqual(first[1], DIAG.paths()[1])
            os.environ['GITHUB_RUN_ID'] = '../private'
            with self.assertRaises(ValueError):
                DIAG.paths()

    def test_optional_failure_never_echoes_private_exception(self):
        with (mock.patch('sys.argv', ['diag', 'snapshot']), mock.patch.object(
            DIAG, 'snapshot', side_effect=ValueError('TOKEN=private')),
            contextlib.redirect_stdout(io.StringIO()) as output):
            DIAG.main()
        self.assertNotIn('TOKEN', output.getvalue())
        self.assertIn('compilation is unaffected', output.getvalue())


if __name__ == '__main__':
    unittest.main()
