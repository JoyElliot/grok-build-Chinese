"""Cloud structural tests; these never claim Windows transaction execution."""
import importlib.util
import pathlib
import unittest

BASE = pathlib.Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('transaction_generator', BASE/'prepare.py')
generator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(generator)

class GeneratorTests(unittest.TestCase):
    def test_transaction_anchors_preserve_crlf_bytes(self):
        raw = b'prefix\r\n        let destination_name = destination\r\nfoo\r\nEND\r\n'
        value = generator.between(raw,b'        let destination_name = destination\n',b'END','current','test')
        self.assertEqual(value,b'        let destination_name = destination\r\nfoo\r\n')
    def test_function_body_preserves_crlf_bytes(self):
        raw=b'fn example() {\r\n    let value = 4;\r\n}\r\n'
        value=generator.exact_item(raw,'example','current')
        self.assertIn(b'fn example() {\r\n    let value = 4;\r\n}',value)
    def test_historical_tamper_is_rejected(self):
        import tempfile
        oldbase=generator.BASE
        with tempfile.TemporaryDirectory() as directory:
            try:
                generator.BASE=pathlib.Path(directory)
                path='crates/codegen/xai-grok-update/src/auto_update.rs'
                target=generator.BASE/'historical'/path;target.parent.mkdir(parents=True);target.write_bytes(b'tampered')
                with self.assertRaisesRegex(RuntimeError,'Historical source changed'):
                    generator.read(path,True)
            finally:
                generator.BASE=oldbase
    def test_rollback_fixture_releases_lock_before_snapshot(self):
        source=(BASE/'current_tests.rs').read_text()
        case=source.split('async fn current_source_activation_failure_preserves_real_native_image()',1)[1]
        self.assertLess(case.index('windows_activate_staged_exe('),case.index('drop(lock);'))
        self.assertLess(case.index('drop(lock);'),case.index('assert_rejection_unchanged('))
    def test_atomic_replacement_has_no_rename_away_fallback(self):
        source=(generator.ROOT/'crates/codegen/xai-grok-update/src/auto_update.rs').read_bytes()
        activation=generator.exact_item(source,'windows_activate_staged_exe','current').decode()
        self.assertIn('tokio::fs::hard_link(dest, &aside)', activation)
        self.assertIn('windows_atomic_replace(staged, dest)', activation)
        self.assertNotIn('windows_rename_no_replace(dest,', activation)
        self.assertNotIn('windows_rename_no_replace(&aside,', activation)
        self.assertNotIn('remove_file(dest)', activation)
        self.assertLess(activation.index('hard_link(dest, &aside)'), activation.index('windows_atomic_replace(staged, dest)'))
    def test_checkpoints_cannot_enable_product_environment_hooks(self):
        source=(generator.ROOT/'crates/codegen/xai-grok-update/src/auto_update.rs').read_text()
        self.assertIn('#[cfg(all(windows, test))]\nfn windows_activation_test_checkpoint', source)
        for phase in ['before-backup','after-backup','after-replace']:
            self.assertIn('#[cfg(test)]\n    windows_activation_test_checkpoint("'+phase+'", dest);', source)
    def test_pre_fix_negative_fixture_is_pinned_and_historical_is_unchanged(self):
        import hashlib, json
        raw=(BASE/'regressions/pre-fix-activation-b3ca1daa.rs').read_bytes()
        pin=json.loads((BASE/'regressions/pre-fix-activation-b3ca1daa.json').read_text())
        self.assertEqual(pin['sha256'], hashlib.sha256(raw).hexdigest())
        self.assertIn(b'windows_rename_no_replace(dest, &aside)', raw)
        self.assertNotIn(b'windows_activation_test_checkpoint', raw)
        generator.read('crates/codegen/xai-grok-update/src/auto_update.rs', True)
    def test_regression_changes_invalidate_generated_build(self):
        build=(BASE/'build.rs').read_text()
        self.assertIn('cargo:rerun-if-changed=regressions', build)
    def test_pinned_regression_sources_disable_checkout_line_conversion(self):
        attributes=(generator.ROOT/'.gitattributes').read_text()
        self.assertIn('.github/scripts/direct-transaction-validation/regressions/** -text', attributes.splitlines())
    def test_fault_injection_is_bounded_and_covers_live_image(self):
        source=(BASE/'atomic_activation_tests.rs').read_text()
        self.assertIn('child.0.kill().unwrap()', source)
        self.assertIn('std::env::current_exe()', source)
        self.assertIn('assert_launchable(expected)', source)
        self.assertIn('for phase in ["before-backup", "after-backup", "after-replace"]', source)
        self.assertIn('Duration::from_secs(20)', source)
        self.assertNotIn('#[ignore', source)
    def test_no_compiled_fixture_absolute_paths(self):
        for filename in ['transaction_tests.rs','current_tests.rs']:
            source=(BASE/filename).read_text()
            self.assertNotIn('CARGO_MANIFEST_DIR',source)
            self.assertNotIn('File::Replace',source)
            self.assertNotIn('previous-executable-boundary-fixture',source)

if __name__=='__main__': unittest.main()
