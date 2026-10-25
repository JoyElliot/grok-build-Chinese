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
    def test_no_compiled_fixture_absolute_paths(self):
        for filename in ['transaction_tests.rs','current_tests.rs']:
            source=(BASE/filename).read_text()
            self.assertNotIn('CARGO_MANIFEST_DIR',source)
            self.assertNotIn('File::Replace',source)
            self.assertNotIn('previous-executable-boundary-fixture',source)

if __name__=='__main__': unittest.main()
