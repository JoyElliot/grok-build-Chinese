#!/usr/bin/env python3
"""Observation accounting tests; these never contact GitHub or launch product EXEs."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('observation', ROOT / '.github/scripts/windows-update-check-observation.py')
observation = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(observation)


class TestObservation(unittest.TestCase):
    def status(self, **changes):
        value = {'currentVersion': '1.0.45', 'latestVersion': '1.0.103', 'updateAvailable': True, 'error': None}
        value.update(changes)
        return json.dumps(value)

    def test_real_shape_is_observation_not_update_success(self):
        outcome, status = observation.parse_status(self.status(), '1.0.45')
        self.assertEqual(outcome, 'observed')
        self.assertTrue(status['updateAvailable'])

    def test_rate_limit_remains_unavailable(self):
        outcome, _ = observation.parse_status(self.status(error='GitHub API returned 403', latestVersion=None), '1.0.45')
        self.assertEqual(outcome, 'unavailable')

    def test_no_latest_is_unavailable_even_with_zero_process_exit(self):
        outcome, _ = observation.parse_status(self.status(latestVersion=None, updateAvailable=False), '1.0.45')
        self.assertEqual(outcome, 'unavailable')

    def test_wrong_current_version_rejected(self):
        with self.assertRaisesRegex(ValueError, 'currentVersion'):
            observation.parse_status(self.status(currentVersion='1.0.103'), '1.0.45')

    def test_malformed_or_incomplete_output_rejected(self):
        for text in ['not json', '[]', '{}', self.status(updateAvailable='yes')]:
            with self.subTest(text=text), self.assertRaises(ValueError):
                observation.parse_status(text, '1.0.45')

    def test_invocation_is_read_only_and_separately_bounded(self):
        text = (ROOT / '.github/scripts/windows-update-check-observation.py').read_text(encoding='utf-8')
        self.assertIn("[str(exe), 'update', '--check', '--json']", text)
        self.assertIn('timeout_seconds=90', text)
        self.assertIn("'compatibility_gate': False", text)
        self.assertNotIn('--force-reinstall', text)
        self.assertNotIn('GH_TOKEN', text)
        self.assertNotIn('os.environ[', text)
        self.assertIn('direct.file_manifest(package) == before', text)

    def test_unconfirmed_termination_is_not_unavailable_network_error(self):
        error = observation.direct.UnconfirmedProcessTermination('still running')
        self.assertNotIsInstance(error, (ValueError, OSError))

    def test_profile_allows_cache_but_rejects_download_and_bootstrap(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / 'version.json').write_text('{}')
            self.assertEqual(observation.profile_files(root), ['version.json'])
            for name in ['payload.ZIP', 'candidate.exe', 'Install-GrokZh.ps1', 'bootstrap.json', 'migration.json']:
                path = root / name
                path.write_text('inert fixture')
                with self.subTest(name=name), self.assertRaisesRegex(ValueError, 'created executable/installer/archive'):
                    observation.profile_files(root)
                path.unlink()


if __name__ == '__main__':
    unittest.main()
