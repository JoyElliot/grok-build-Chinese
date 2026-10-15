"""Protect coverage and isolation while Windows validation is split across jobs."""

import json
from pathlib import Path
import re
import unittest

from test_release_workflow import dependencies, job_blocks

ROOT = Path(__file__).resolve().parents[3]


class WindowsValidationTests(unittest.TestCase):
    def test_msvc_runs_both_unmodified_shared_suites_on_one_native_runner(self):
        jobs = job_blocks((ROOT / '.github/workflows/zh-dev-windows-preview.yml').read_text(encoding='utf-8'))
        native = jobs['windows-msvc-migration-test']
        self.assertIn('runs-on: windows-2022', native)
        self.assertIn('RUSTUP_TOOLCHAIN: 1.94.0-x86_64-pc-windows-msvc', native)
        self.assertIn('TARGET: x86_64-pc-windows-msvc', native)
        self.assertIn('CARGO_PROFILE_TEST_DEBUG: "0"', native)
        self.assertIn('CARGO_INCREMENTAL: "0"', native)
        self.assertIn('timeout-minutes: 180', native)
        self.assertNotIn('matrix:', native)  # Share common dependency builds between core and ui.
        self.assertIn('phase: test', native)
        self.assertEqual(native.count('Test-WindowsMigration.ps1'), 2)
        self.assertNotIn('continue-on-error', native)
        self.assertIn('python3 .github/scripts/tests/test_windows_validation.py', jobs['localization'])

        action = (ROOT / '.github/actions/build-windows-msvc-x64/action.yml').read_text(encoding='utf-8')
        steps = re.split(r'^    - ', action, flags=re.M)
        execute = [step for step in steps if "foreach ($suite in @('core', 'ui'))" in step]
        self.assertEqual(len(execute), 1)
        execute = execute[0]
        self.assertEqual(re.findall(r'^ +if: (.+)$', execute, re.M), ["inputs.phase != 'build'"])
        self.assertIn("inputs.validation-mode || (inputs.release_build == 'true' && 'release' || 'preview')", execute)
        self.assertIn("$env:TARGET -cne 'x86_64-pc-windows-msvc'", execute)
        self.assertIn("$rustInfo -cnotcontains 'host: x86_64-pc-windows-msvc'", execute)
        self.assertIn('run-windows-validation.ps1', execute)
        self.assertIn('-Mode $env:VALIDATION_MODE -Suite $suite -FormatScope suite', execute)
        self.assertIn('-arch=x64 -host_arch=x64', execute)
        self.assertIn('if ($LASTEXITCODE -ne 0) { throw', execute)
        for bypass in ('cache-hit', 'continue-on-error', '-SkipResourceMonitor', 'cargo test ', 'cargo build '):
            self.assertNotIn(bypass, execute)
        # In particular, do not retain an extra standalone updater compilation:
        # its complete --lib invocation is already part of each core manifest.
        for mode in ('preview', 'release'):
            manifest = json.loads((ROOT / '.github/scripts/windows-validation-tests.json').read_text())
            self.assertEqual(manifest[mode]['core'].count(
                ['-p', 'xai-grok-update', '--features', 'community-build', '--lib']), 1)

    def test_msvc_parity_is_mandatory_and_diagnostics_cover_both_suites(self):
        jobs = job_blocks((ROOT / '.github/workflows/zh-dev-windows-preview.yml').read_text(encoding='utf-8'))
        gate = jobs['multiplatform-result']
        self.assertIn('windows-msvc-migration-test', dependencies(gate))
        self.assertIn('MIGRATION_TEST_RESULT: ${{ needs.windows-msvc-migration-test.result }}', gate)
        self.assertIn('"${MIGRATION_TEST_RESULT}" != success', gate)
        self.assertIn('always()', gate)
        action = (ROOT / '.github/actions/build-windows-msvc-x64/action.yml').read_text(encoding='utf-8')
        diagnostics = [step for step in re.split(r'^    - ', action, flags=re.M)
                       if 'uses: actions/upload-artifact@' in step and 'if-no-files-found: warn' in step]
        self.assertEqual(len(diagnostics), 2)
        for diagnostic in diagnostics:
            for path in ('grok-zh-target/cargo-timings', 'grok-zh-windows-validation-core',
                         'grok-zh-windows-validation-ui'):
                self.assertIn(path, diagnostic)
        self.assertIn("failure() && !cancelled() && inputs.release_build != 'true'", diagnostics[-1])
        monitor = (ROOT / '.github/scripts/windows-build-resource-monitor.ps1').read_text(encoding='utf-8')
        for tool in ('cargo', 'rustc', 'gcc', 'cl', 'link', 'lib'):
            self.assertIn(f"'{tool}'", monitor)

    def test_shared_runner_preserves_gnu_format_and_msvc_package_checks(self):
        runner = (ROOT / '.github/scripts/run-windows-validation.ps1').read_text(encoding='utf-8')
        self.assertIn("[ValidateSet('workspace', 'suite')][string]$FormatScope = 'workspace'", runner)
        self.assertIn("if ($FormatScope -eq 'suite')", runner)
        self.assertIn('& cargo fmt -p $package -- --check', runner)
        self.assertIn("elseif ($Suite -eq 'core')", runner)
        self.assertIn('& cargo fmt --all -- --check', runner)
        self.assertIn('@($manifest.$Mode.$Suite)', runner)
        self.assertIn("@('test', '--frozen', '--timings', '-j', $env:CARGO_BUILD_JOBS) + @($selection)", runner)
        self.assertIn('$PSNativeCommandUseErrorActionPreference = $false', runner)
        self.assertIn('if ($exitCode -ne 0) { throw', runner)
        self.assertIn('target = $env:TARGET', runner)
        self.assertIn('format_scope = $FormatScope', runner)

    def test_runner_fixture_uses_native_cargo_and_checks_literal_format_separator(self):
        fixture = (ROOT / '.github/scripts/tests/Test-WindowsValidation.ps1').read_text(encoding='utf-8')
        self.assertNotRegex(fixture, r'(?im)^\s*function\s+cargo\b')
        self.assertIn("$fixture = Join-Path $temp 'cargo.exe'", fixture)
        self.assertIn('Microsoft.NET/Framework64/v4.0.30319/csc.exe', fixture)
        self.assertIn("$resolvedCargo.CommandType -ne 'Application'", fixture)
        self.assertIn('$resolvedCargo.Source -ne $fixture', fixture)
        self.assertIn('File.AppendAllLines(log, new[] { string.Join(" ", args) }', fixture)
        self.assertIn('return File.ReadAllLines(log).Length == failAt ? 17 : 0;', fixture)
        self.assertIn('& cargo fmt --all -- --check', fixture)
        self.assertIn('& cargo fmt -p fixture-package -- --check', fixture)
        self.assertIn("@('fmt --all -- --check', 'fmt -p fixture-package -- --check')", fixture)
        self.assertIn('$calls[$index] -cne $formatCommands[$index]', fixture)
        self.assertIn('Expected: [$($formatCommands[$index])]; actual: [$($calls[$index])]', fixture)
        self.assertIn('Reset-CargoFixture -FailAt 1', fixture)
        self.assertIn('Reset-CargoFixture -FailAt 2', fixture)
        self.assertIn('Reset-CargoFixture -FailAt ($formatCount + 2)', fixture)
        self.assertIn('foreach ($name in $variables) { [Environment]::SetEnvironmentVariable($name, $previous[$name]) }', fixture)

    def test_migration_fixture_delimits_variables_before_chinese_diagnostics(self):
        fixture = (ROOT / 'packaging/windows/tests/Test-WindowsMigration.ps1').read_text(encoding='utf-8-sig')
        # Han letters are part of an unbraced PowerShell variable name, so
        # StrictMode fails while evaluating an assertion's diagnostic message.
        unsafe = [(number, line.strip()) for number, line in enumerate(fixture.splitlines(), 1)
                  if re.search(r'\$[A-Za-z_][A-Za-z0-9_]*[\u3400-\u9fff]', line)]
        self.assertEqual(unsafe, [], f'Unbraced variables adjacent to Chinese text: {unsafe}')
        self.assertIn('${versionArgument}显示连续更新后的真实子程序', fixture)
        self.assertIn('${futureVersion}后正常启动不依赖TEMP', fixture)
        self.assertIn("foreach ($versionArgument in @('--version','-V'))", fixture)
        self.assertIn("foreach ($futureVersion in @('1.0.100','1.0.101'))", fixture)

    def test_existing_test_contracts_remain_separate(self):
        suites = json.loads((ROOT / '.github/scripts/windows-validation-tests.json').read_text())
        common = {
            ('-p', 'xai-grok-update', '--features', 'community-build', '--lib'),
            ('-p', 'xai-grok-locale'),
            ('-p', 'xai-grok-agent', '--lib', 'prompt::'),
            ('-p', 'xai-grok-tools', '--lib', 'implementations::use_tool'),
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
            required = {prefix + '-rust-validation'}
            if filename.startswith('zh-release'):
                required.add('release-plan')
            self.assertEqual(dependencies(gate), required)
            if filename.startswith('zh-release'):
                self.assertIn('if: always()', gate)
            else:
                self.assertIn(
                    "if: ${{ always() && !(github.event_name == 'push' && startsWith(github.ref, 'refs/heads/sync/')) }}",
                    gate,
                )
            self.assertIn('${{ toJSON(needs.*.result) }}', gate)
            self.assertIn('all(. == "success")', gate)
            rust = jobs[prefix + '-rust-validation']
            self.assertNotIn(prefix + '-static-validation', jobs)
            # Static checks run once on core and must succeed before Rust setup.
            checks = [step for step in re.split(r'^      - ', rust, flags=re.M)
                      if 'run: ./.github/scripts/check-windows-package.ps1' in step]
            self.assertEqual(len(checks), 1)
            self.assertIn("if: matrix.suite == 'core'", checks[0])
            self.assertIn('timeout-minutes: 15', checks[0])
            self.assertNotIn('continue-on-error:', checks[0])
            mode = 'release' if filename.startswith('zh-release') else 'preview'
            self.assertIn(f'check-windows-package.ps1 -Mode {mode}', checks[0])
            self.assertLess(rust.index('check-windows-package.ps1'),
                            rust.index('uses: ./.github/actions/setup-windows-gnu'))
            self.assertIn('fail-fast: false', rust)
            self.assertIn('suite: [core, ui]', rust)
            self.assertIn('suite: ${{ matrix.suite }}', rust)
            if filename.startswith('zh-release'):
                self.assertIn("save-cache: 'false'", rust)
            else:
                self.assertIn("save-cache: ${{ env.GROK_ZH_CACHE_WRITE }}", rust)

    def test_cache_hit_does_not_skip_tests_and_suites_have_distinct_keys(self):
        action = (ROOT / '.github/actions/validate-windows-gnu/action.yml').read_text(encoding='utf-8')
        execute = action.split('- name: 验证并记录')[1].split('- name: 上传')[0]
        self.assertNotIn('if:', execute)
        self.assertIn('run-windows-validation.ps1', execute)
        self.assertIn('debug0-incremental0-${{ inputs.suite }}-', action)
        # v2 keeps the key and every restore-key away from unpruned v1 zh-dev entries.
        restore = action.split('- name: 恢复测试编译缓存', 1)[1].split('- name: 验证并记录', 1)[0]
        prefixes = re.findall(r'grok-zh-test-v\d+-', restore)
        self.assertEqual(len(prefixes), 4)  # key + three restore-keys
        self.assertEqual(set(prefixes), {'grok-zh-test-v2-'})
        self.assertNotIn('grok-zh-test-v1', action)
        self.assertIn("inputs.save-cache == 'true'", action)
        self.assertIn('key: ${{ steps.cache.outputs.cache-primary-key }}', action)
        self.assertNotIn('github.sha', action)  # Bound immutable cache count per configuration.
        # Workspace test binaries are rebuilt every run; only third-party outputs are saved.
        save = action.split('uses: ./.github/actions/save-cargo-cache', 1)[1]
        self.assertIn('prune-profile-dirs: |\n          ${{ runner.temp }}/grok-zh-target/debug', save)

    def test_preview_cross_build_requires_native_packaging_and_all_native_tests(self):
        jobs = job_blocks((ROOT / '.github/workflows/zh-dev-windows-preview.yml').read_text(encoding='utf-8'))
        cross = jobs['windows-gnu-cross-build']
        native = jobs['windows-gnu-build']
        self.assertIn('runs-on: ubuntu-24.04', cross)
        self.assertIn('RUSTUP_TOOLCHAIN: 1.94.0-x86_64-unknown-linux-gnu', cross)
        self.assertIn('TARGET: x86_64-pc-windows-gnu', cross)
        self.assertEqual(dependencies(cross), {'zh-dev-priority-gate', 'localization', 'rust-format-preflight'})
        self.assertIn('runs-on: windows-2022', native)
        self.assertEqual(dependencies(native), {'windows-gnu-cross-build'})
        self.assertIn('needs.windows-gnu-cross-build.outputs.artifact_name', native)
        for identity in ('commit', 'version', 'target', 'profile', 'features', 'build_host', 'sha256'):
            self.assertIn(f'$metadata.{identity}', native)
        self.assertIn('strip-windows-binary.py', native)
        self.assertIn('write-package-protocol.py', native)
        self.assertIn('$env:PATH = "$env:SystemRoot\\System32;$env:SystemRoot"', native)
        self.assertNotIn('continue-on-error:', cross + native)
        self.assertEqual(dependencies(jobs['windows-gnu-rust-validation']), {'zh-dev-priority-gate', 'localization', 'rust-format-preflight'})
        self.assertEqual(dependencies(jobs['windows-gnu-preview']),
                         {'windows-gnu-validation', 'windows-gnu-build'})
        self.assertIn('windows-gnu-preview', dependencies(jobs['multiplatform-result']))
        action = (ROOT / '.github/actions/build-windows-gnu-cross/action.yml').read_text(encoding='utf-8')
        self.assertIn('--profile release-dist --features release-dist', action)
        self.assertIn('--timings --config profile.release-dist.debug=0', action)
        self.assertNotIn('cargo test ', action)


if __name__ == '__main__':
    unittest.main()
