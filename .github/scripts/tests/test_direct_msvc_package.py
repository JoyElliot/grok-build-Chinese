"""Direct MSVC payload identity, old transport contract and fail-closed packaging."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import zipfile
from unittest.mock import patch

import test_embedded_msvc_bundle as fixtures
import test_migration_package

spec = importlib.util.spec_from_file_location('direct', fixtures.SCRIPTS / 'build-windows-direct-msvc.py')
direct = importlib.util.module_from_spec(spec)
spec.loader.exec_module(direct)
VERSION = fixtures.VERSION


class DirectMsvcPackageTests(unittest.TestCase):
    def fixture(self, root, extra=False):
        payload, unused = test_migration_package.MigrationPackageTests().fixture(root)
        (payload / 'grok-zh.exe').write_bytes(fixtures.fake_pe_prefix())
        if not extra:
            (payload / 'future-resource.dat').unlink()
        test_migration_package.MigrationPackageTests().rehash(payload)
        archive = fixtures.builder.native_archive_from_directory(unused, payload, VERSION)
        return payload, archive

    def test_windows_smoke_bounds_process_and_pipe_wait_and_preserves_uncertain_work(self):
        text = (fixtures.builder.ROOT / "packaging/windows/tests/Test-DirectMsvcProduct.ps1").read_text(encoding="utf-8-sig")
        self.assertIn("WaitForExit(30000)", text)
        self.assertIn("WaitForExit(10000)", text)
        self.assertIn("Task]::WaitAll([Threading.Tasks.Task[]]@($stdout, $stderr), 10000)", text)
        self.assertGreaterEqual(text.count("$script:CleanupSafe = $false"), 2)
        self.assertIn("if ($script:CleanupSafe -and (Test-Path -LiteralPath $work))", text)
        self.assertNotIn("taskkill.exe", text)
        for name in ("HOME", "USERPROFILE", "APPDATA", "LOCALAPPDATA"):
            self.assertIn("$env:" + name, text)

    def test_windows_smoke_compares_raw_persistent_path_and_exact_file_set(self):
        text = (fixtures.builder.ROOT / "packaging/windows/tests/Test-DirectMsvcProduct.ps1").read_text(encoding="utf-8-sig")
        self.assertIn("RegistryValueOptions]::DoNotExpandEnvironmentNames", text)
        self.assertIn("$key.GetValueKind('Path')", text)
        self.assertIn("$key.GetValueNames() -icontains 'Path'", text)
        self.assertIn("$userPath = Get-RawUserPathSnapshot", text)
        self.assertIn("if ((Get-RawUserPathSnapshot) -cne $userPath)", text)
        self.assertNotIn("$userPath = [Environment]::GetEnvironmentVariable", text)
        self.assertIn("Compare-Object -ReferenceObject $expectedNames -DifferenceObject $afterNames -CaseSensitive", text)
        self.assertIn("-File -Recurse -Force", text)
        self.assertIn("$_.SideIndicator", text)
        self.assertIn("$_.InputObject.Substring", text)

    def test_exact_native_executable_and_legacy_transport(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            payload, archive = self.fixture(root)
            result = direct.build_package(root / 'direct', archive, VERSION)
            checked = direct.validate_direct_archive(result, VERSION)
            self.assertEqual(set(checked['hashes']), set(direct.LEGACY_FILES))
            self.assertEqual(checked['protocol']['direct_runtime'],
                             dict(schema=1, version=VERSION, platform=direct.bundle.TARGET))
            self.assertNotIn('embedded_runtime', checked['protocol'])
            with zipfile.ZipFile(result) as zipped:
                prefix = result.stem + '/'
                self.assertEqual(zipped.read(prefix + 'grok-zh.exe'), (payload / 'grok-zh.exe').read_bytes())
                # Old parsers ignore the extension; schema/platform/mode and
                # exact legacy manifest remain unchanged.
                self.assertEqual(checked['protocol']['platform'], 'x86_64-pc-windows-gnu')
                self.assertEqual(checked['protocol']['mode'], 'executable-only')
                for member in set(direct.LEGACY_FILES) - {'BUILD-INFO.txt'}:
                    self.assertEqual(zipped.read(prefix + member), (payload / member).read_bytes())
            self.assertEqual(result.with_suffix('.zip.sha256').read_text(),
                             hashlib.sha256(result.read_bytes()).hexdigest() + '  ' + result.name + '\n')
            again = direct.build_package(root / 'again', archive, VERSION)
            self.assertEqual(result.read_bytes(), again.read_bytes())

    def test_extra_dependency_requires_review_before_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, archive = self.fixture(root, extra=True)
            with self.assertRaises(ValueError):
                direct.build_package(root / 'direct', archive, VERSION)
            self.assertFalse((root / 'direct').exists())

    def test_existing_output_untouched(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, archive = self.fixture(root)
            output = root / 'direct'; output.mkdir(); (output / 'keep').write_text('keep')
            with self.assertRaises(FileExistsError):
                direct.build_package(output, archive, VERSION)
            self.assertEqual(list(output.iterdir()), [output / 'keep'])

    def test_tampered_archive_rejected_before_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, archive = self.fixture(root)
            broken = fixtures.EmbeddedMsvcBundleTests().mutate_zip(archive, lambda entries:
                [(entry, b'tampered' if entry.filename.endswith('/rg.exe') else data) for entry, data in entries])
            with self.assertRaises(ValueError):
                direct.build_package(root / 'direct', broken, VERSION)
            self.assertFalse((root / 'direct').exists())

    def test_archive_change_after_validation_cannot_bless_companion(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); _, archive = self.fixture(root)
            original_validate = direct.bundle.validate_native_archive
            def replace_after_validation(path, version):
                result = original_validate(path, version)
                changed = fixtures.EmbeddedMsvcBundleTests().mutate_zip(path, lambda entries:
                    [(entry, b"changed installer" if entry.filename.endswith("/Install-GrokZh.ps1") else data)
                     for entry, data in entries])
                changed.replace(path)
                return result
            with patch.object(direct.bundle, "validate_native_archive", side_effect=replace_after_validation):
                with self.assertRaisesRegex(ValueError, "changed after validation"):
                    direct.build_package(root / "direct", archive, VERSION)
            self.assertFalse((root / "direct").exists())
            self.assertFalse(list(root.glob(".direct-msvc-*")))

    def test_unsafe_member_rejected(self):
        for member in ('../escape', 'grok-zh-1.0.99-windows-x86_64-msvc/../escape',
                       'grok-zh-1.0.99-windows-x86_64-msvc/CON.txt'):
            with self.subTest(member=member), tempfile.TemporaryDirectory() as directory:
                root = Path(directory); _, archive = self.fixture(root)
                broken = fixtures.EmbeddedMsvcBundleTests().mutate_zip(archive, lambda entries:
                    entries + [(member, b'bad')])
                with self.assertRaises(ValueError):
                    direct.build_package(root / 'direct', broken, VERSION)
                self.assertFalse((root / 'direct').exists())

    def test_invalid_direct_protocol_rejected(self):
        valid = dict(schema=1, version=VERSION, platform='x86_64-pc-windows-gnu',
                     mode='executable-only', manifest='SHA256SUMS.txt', executable='grok-zh.exe',
                     installer='Install-GrokZh.ps1', direct_runtime=dict(schema=1, version=VERSION, platform=direct.bundle.TARGET))
        variants = [valid | {'embedded_runtime': {}}, valid | {'direct_runtime': None},
                    valid | {'direct_runtime': valid['direct_runtime'] | {'schema': True}},
                    valid | {'direct_runtime': valid['direct_runtime'] | {'version': '1.0.98'}},
                    valid | {'direct_runtime': valid['direct_runtime'] | {'platform': 'x86_64-pc-windows-gnu'}}]
        for protocol in variants:
            text = 'Target: x86_64-pc-windows-gnu\nGROK-UPDATE-PROTOCOL-BEGIN\n' + json.dumps(protocol) + '\nGROK-UPDATE-PROTOCOL-END\n'
            with self.subTest(protocol=protocol), self.assertRaises(ValueError):
                direct.bundle.protocol_from_text(text, VERSION, direct=True)

    def test_mislabeled_embedded_payload_rejected_and_stage_cleaned(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); payload, archive = self.fixture(root)
            # Not executed: append just the reserved footer signature to a PE
            # fixture to ensure the packaging route cannot hide a known wrapper.
            (payload / 'grok-zh.exe').write_bytes(fixtures.fake_pe_prefix() + direct.bundle.MAGIC + bytes(48))
            test_migration_package.MigrationPackageTests().rehash(payload)
            archive.unlink()
            archive = fixtures.builder.native_archive_from_directory(archive.parent, payload, VERSION)
            with self.assertRaises(ValueError):
                direct.build_package(root / 'direct', archive, VERSION)
            self.assertFalse((root / 'direct').exists())
            self.assertFalse(list(root.glob('.direct-msvc-*')))


if __name__ == '__main__':
    unittest.main()
