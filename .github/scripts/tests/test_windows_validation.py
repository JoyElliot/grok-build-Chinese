"""Protect coverage and isolation while Windows validation is split across jobs."""

import json
from pathlib import Path
import unittest

from test_release_workflow import dependencies, job_blocks

ROOT = Path(__file__).resolve().parents[3]


class WindowsValidationTests(unittest.TestCase):
    def test_existing_test_contracts_remain_separate(self):
        suites = json.loads((ROOT / '.github/scripts/windows-validation-tests.json').read_text())
        common = {
            ('-p', 'xai-grok-update', '--features', 'community-build', '--lib'),
            ('-p', 'xai-grok-locale'),
            ('-p', 'xai-grok-shell', '--lib', 'session_summary'),
            ('-p', 'xai-grok-shell', '--lib', 'builtin::tests::'),
            ('-p', 'xai-grok-shell', '--lib', 'bundled_model_provenance_survives_acp_but_config_override_clears_it'),
            ('-p', 'xai-grok-pager', '--lib', 'zh_localization_'),
            ('-p', 'xai-grok-pager-minimal', '--lib', 'zh_localization_'),
            ('-p', 'xai-grok-pager', '--lib', 'free_usage_upsell_shows_three_options_with_exact_labels'),
        }
        preview = {
            ('-p', 'xai-grok-shell', '--lib', 'hook_annotation_'),
            ('-p', 'xai-grok-shell', '--lib', 'prompt_suggest_catalog_check_honors_allowlist'),
            ('-p', 'xai-grok-pager', '--lib', 'scrollback::render::tests::'),
            ('-p', 'xai-grok-pager', '--lib', 'views::shortcuts_help::tests::'),
        }
        release = {
            ('-p', 'xai-grok-product'), ('-p', 'xai-grok-version'),
            ('-p', 'xai-grok-config', '--lib', 'version_match_boundaries'),
            ('-p', 'xai-grok-shell', '--lib', 'leader_is_older_than_directional'),
            ('-p', 'xai-grok-pager', '--lib', 'chinese_effort_rows_use_typed_values_for_all_tiers_and_keep_option_ids'),
            ('-p', 'xai-grok-pager', '--lib', 'localization_regression_grok_announcements_cover_4_5_and_4_6'),
            ('-p', 'xai-grok-pager', '--lib', 'localization_regression_slash_snapshot_uses_canonical_metadata_only'),
        }
        for mode, extra in [('preview', preview), ('release', release)]:
            actual = [tuple(command) for commands in suites[mode].values() for command in commands]
            self.assertEqual(set(actual), common | extra)
            self.assertEqual(len(actual), len(common | extra))
            for suite, commands in suites[mode].items():
                for command in commands:
                    self.assertEqual(command.count('-p'), 1)  # No feature union across packages.
                    self.assertEqual(command[1].startswith('xai-grok-pager'), suite == 'ui')

    def test_aggregates_wait_for_both_shards_and_static_checks(self):
        for filename, prefix in [('zh-dev-windows-preview.yml', 'windows-gnu'), ('zh-release-windows.yml', 'windows-x64-gnu')]:
            jobs = job_blocks((ROOT / '.github/workflows' / filename).read_text(encoding='utf-8'))
            gate = jobs[prefix + '-validation']
            required = {prefix + '-static-validation', prefix + '-rust-validation'}
            if filename.startswith('zh-release'):
                required.add('release-plan')
            self.assertEqual(dependencies(gate), required)
            self.assertIn('if: always()', gate)
            self.assertIn('${{ toJSON(needs.*.result) }}', gate)
            self.assertIn('all(. == "success")', gate)
            rust = jobs[prefix + '-rust-validation']
            self.assertIn('fail-fast: false', rust)
            self.assertIn('suite: [core, ui]', rust)
            self.assertIn('suite: ${{ matrix.suite }}', rust)
            if filename.startswith('zh-release'):
                self.assertIn("save-cache: 'false'", rust)
            else:
                self.assertIn('github.event.pull_request.head.repo.full_name == github.repository', rust)

    def test_cache_hit_does_not_skip_tests_and_suites_have_distinct_keys(self):
        action = (ROOT / '.github/actions/validate-windows-gnu/action.yml').read_text(encoding='utf-8')
        execute = action.split('- name: 验证并记录')[1].split('- name: 上传')[0]
        self.assertNotIn('if:', execute)
        self.assertIn('run-windows-validation.ps1', execute)
        self.assertIn('debug0-incremental0-${{ inputs.suite }}-', action)
        self.assertIn("inputs.save-cache == 'true'", action)
        self.assertIn('key: ${{ steps.cache.outputs.cache-primary-key }}', action)
        self.assertNotIn('github.sha', action)  # Bound immutable cache count per configuration.


if __name__ == '__main__':
    unittest.main()
