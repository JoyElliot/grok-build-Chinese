"""Offline format, pinning, safe ZIP and legacy transport regressions."""
import copy
import hashlib
import importlib.util
import json
import random
from pathlib import Path
import struct
import tempfile
import unittest
import zipfile
from unittest.mock import patch

import test_migration_package

SCRIPTS = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("embedded_builder", SCRIPTS / "build-windows-embedded-bootstrap.py")
builder = importlib.util.module_from_spec(spec)
spec.loader.exec_module(builder)
bundle = builder.bundle
VERSION = "1.0.99"


def fake_pe_prefix():
    data = bytearray(512)
    data[:2] = b"MZ"
    struct.pack_into("<I", data, 0x3C, 64)
    data[64:68] = b"PE\0\0"
    struct.pack_into("<H", data, 68, 0x8664)
    struct.pack_into("<H", data, 70, 1)
    struct.pack_into("<H", data, 84, 240)
    struct.pack_into("<H", data, 88, 0x20B)
    return bytes(data)


class EmbeddedMsvcBundleTests(unittest.TestCase):
    def fixture(self, root):
        payload, output = test_migration_package.MigrationPackageTests().fixture(root)
        (output / "grok-zh.exe").unlink()
        (payload / "grok-zh.exe").write_bytes(fake_pe_prefix())
        test_migration_package.MigrationPackageTests().rehash(payload)
        native = builder.native_archive_from_directory(output, payload, VERSION)
        prefix = root / "prefix.exe"
        prefix.write_bytes(fake_pe_prefix())
        launcher = output / "grok-zh.exe"
        metadata = bundle.append_bundle(prefix, native, launcher, VERSION)
        return payload, output, native, launcher, metadata

    def mutate_zip(self, archive, changes):
        with zipfile.ZipFile(archive) as zipped:
            entries = [(entry, zipped.read(entry)) for entry in zipped.infolist()]
        replacement = archive.with_suffix(".changed.zip")
        with zipfile.ZipFile(replacement, "w", zipfile.ZIP_DEFLATED) as zipped:
            for entry, data in changes(entries):
                zipped.writestr(entry, data)
        return replacement

    def test_version_is_bounded_to_canonical_rust_semver(self):
        for version in ("1.0.00", "1.0.0-alpha.1", "1.0." + "9" * 65, "18446744073709551616.0.0"):
            with self.subTest(version=version), self.assertRaises(ValueError):
                bundle.canonical_version(version)
        self.assertEqual(bundle.canonical_version("18446744073709551615.0.0"), "18446744073709551615.0.0")

    def test_footer_exact_layout_and_payload_occurs_once(self):
        with tempfile.TemporaryDirectory() as directory:
            _, _, native, launcher, metadata = self.fixture(Path(directory))
            data = launcher.read_bytes()
            self.assertEqual(len(bundle.MAGIC), 16)
            self.assertEqual(bundle.FOOTER.size, 64)
            magic, offset, size, digest = struct.unpack("<16sQQ32s", data[-64:])
            self.assertEqual(magic, b"GROKMSVCBUNDLE01")
            self.assertEqual(data[offset:offset + size], native.read_bytes())
            self.assertEqual(len(data), offset + size + 64)
            self.assertEqual(digest.hex(), metadata["archive_sha256"])
            self.assertEqual(data.count(native.read_bytes()), 1)
            self.assertEqual(bundle.validate_bundle(launcher, VERSION)["archive_sha256"], digest.hex())

    def test_bad_magic_truncation_trailing_bytes_offsets_sizes_and_digest_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, _, _, launcher, _ = self.fixture(root)
            original = launcher.read_bytes()
            magic, offset, size, digest = bundle.FOOTER.unpack(original[-64:])
            bad_footers = ((b"wrong".ljust(16, b"\0"), offset, size, digest),
                           (magic, 0, size, digest), (magic, offset + 1, size, digest),
                           (magic, 2**64 - 1, size, digest), (magic, offset, 0, digest),
                           (magic, offset, bundle.MAX_ARCHIVE_BYTES + 1, digest),
                           (magic, offset, size, b"\0" * 32))
            cases = [original[:-1], original + b"trailing", b"not an exe"]
            cases += [original[:-64] + bundle.FOOTER.pack(*fields) for fields in bad_footers]
            tampered = bytearray(original)
            tampered[offset + 20] ^= 1
            cases.append(bytes(tampered))
            for index, data in enumerate(cases):
                candidate = root / f"bad-{index}.exe"
                candidate.write_bytes(data)
                with self.subTest(index=index), self.assertRaises(ValueError):
                    bundle.read_footer(candidate)

    def test_pe_prefix_bounds_machine_and_optional_header_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, _, _, launcher, _ = self.fixture(root)
            original = launcher.read_bytes()
            for position, replacement in ((0, b"NO"), (0x3C, struct.pack("<I", 0)),
                    (0x3C, struct.pack("<I", 500)), (64, b"NOPE"), (68, b"\x4c\x01"),
                    (84, b"\x01\x00"), (84, b"\xff\xff"), (70, b"\x00\x00"), (70, b"\xff\xff"), (88, b"\x0b\x01")):
                data = bytearray(original)
                data[position:position + len(replacement)] = replacement
                launcher.write_bytes(data)
                with self.subTest(position=position), self.assertRaises(ValueError):
                    bundle.read_footer(launcher, verify=False)

    def test_external_pins_must_match_even_a_self_consistent_footer(self):
        with tempfile.TemporaryDirectory() as directory:
            _, _, _, launcher, metadata = self.fixture(Path(directory))
            for kwargs in ({"expected_size": metadata["archive_size"] + 1}, {"expected_sha256": "0" * 64}):
                with self.assertRaises(ValueError):
                    bundle.read_footer(launcher, **kwargs)

    def test_extract_streams_and_removes_only_owned_failed_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, _, native, launcher, metadata = self.fixture(root)
            destination = root / "payload.zip"
            bundle.extract_archive(launcher, destination, expected_size=metadata["archive_size"], expected_sha256=metadata["archive_sha256"])
            self.assertEqual(destination.read_bytes(), native.read_bytes())
            with self.assertRaises(FileExistsError):
                bundle.extract_archive(launcher, destination)
            self.assertEqual(destination.read_bytes(), native.read_bytes())
            destination.unlink()
            data = bytearray(launcher.read_bytes())
            data[-70] ^= 1
            launcher.write_bytes(data)
            with self.assertRaises(ValueError):
                bundle.extract_archive(launcher, destination)
            self.assertFalse(destination.exists())

    def test_native_manifest_hash_version_target_and_extra_files_are_checked(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            _, _, native, _, _ = self.fixture(root)
            prefix = f"grok-zh-{VERSION}-windows-x86_64-msvc/"
            transforms = [
                lambda entries: [(e, b"tamper" if e.filename.endswith("grok-zh.exe") else d) for e, d in entries],
                lambda entries: entries + [(prefix + "unlisted.txt", b"extra")],
                lambda entries: [(e, d.replace(b"x86_64-pc-windows-msvc", b"x86_64-pc-windows-gnu") if e.filename.endswith("BUILD-INFO.txt") else d) for e, d in entries],
                lambda entries: [(e, d.replace(b"1.0.99", b"1.0.98") if e.filename.endswith("BUILD-INFO.txt") else d) for e, d in entries],
            ]
            for transform in transforms:
                changed = self.mutate_zip(native, transform)
                with self.assertRaises(ValueError):
                    bundle.validate_native_archive(changed, VERSION)

    def test_authenticated_inner_executable_must_be_native_x64_pe(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            payload, output, native, _, _ = self.fixture(root)
            for executable in (b"wrong executable", fake_pe_prefix()[:68] + b"\x4c\x01" + fake_pe_prefix()[70:]):
                (payload / "grok-zh.exe").write_bytes(executable)
                test_migration_package.MigrationPackageTests().rehash(payload)
                native.unlink()
                with self.assertRaises(ValueError):
                    builder.native_archive_from_directory(output, payload, VERSION)

    def test_unsafe_duplicate_symlink_and_unlisted_directory_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            _, _, native, _, _ = self.fixture(Path(directory))
            prefix = f"grok-zh-{VERSION}-windows-x86_64-msvc/"
            for member in ("../escape", "/absolute", "stream:ads", "CON.txt", "trailing. ", "a//b", "a\\b", "empty/"):
                with self.subTest(member=member):
                    changed = self.mutate_zip(native, lambda entries: entries + [(prefix + member, b"")])
                    with self.assertRaises(ValueError):
                        bundle.validate_native_archive(changed, VERSION)
            duplicate = self.mutate_zip(native, lambda entries: entries + [(prefix + "GROK-ZH.EXE", b"duplicate")])
            with self.assertRaises(ValueError):
                bundle.validate_native_archive(duplicate, VERSION)
            link = zipfile.ZipInfo(prefix + "link")
            link.external_attr = 0o120777 << 16
            linked = self.mutate_zip(native, lambda entries: entries + [(link, b"grok-zh.exe")])
            with self.assertRaises(ValueError):
                bundle.validate_native_archive(linked, VERSION)

    def test_protocol_json_duplicates_boolean_schema_and_nested_runtime_rejected(self):
        protocol = dict(schema=1, version=VERSION, platform=bundle.TARGET, mode="executable-only",
                        manifest="SHA256SUMS.txt", executable="grok-zh.exe", installer="Install-GrokZh.ps1")
        for invalid in (json.dumps(protocol).replace('"schema": 1', '"schema": true'),
                        json.dumps(protocol).replace('"schema": 1', '"schema": 1, "schema": 1'),
                        json.dumps(protocol | {"embedded_runtime": {}})):
            text = f"Target: {bundle.TARGET}\nGROK-UPDATE-PROTOCOL-BEGIN\n{invalid}\nGROK-UPDATE-PROTOCOL-END\n"
            with self.assertRaises(ValueError):
                bundle.protocol_from_text(text, VERSION)

    def test_outer_legacy_exact_files_and_full_inner_additional_resource(self):
        with tempfile.TemporaryDirectory() as directory:
            payload, output, native, _, metadata = self.fixture(Path(directory))
            archive = builder.build_compat_package(output, payload, VERSION, archive=native)
            result = bundle.validate_compat_archive(archive, VERSION)
            self.assertEqual(result["embedded_runtime"], metadata)
            self.assertEqual(set(result["outer"]["hashes"]), set(builder.LEGACY_FILES))
            self.assertIn("future-resource.dat", result["native"]["hashes"])
            self.assertNotIn("future-resource.dat", result["outer"]["hashes"])
            with zipfile.ZipFile(archive) as zipped:
                self.assertEqual(len(zipped.namelist()), 16)
                text = zipped.read(result["outer"]["root"] + "/BUILD-INFO.txt").decode()
                self.assertIn("Target: x86_64-pc-windows-gnu", text)
                self.assertNotIn("downloaded once", text)

    def test_tampered_payload_directory_fails_before_creating_outer_tree(self):
        with tempfile.TemporaryDirectory() as directory:
            payload, output, _, _, _ = self.fixture(Path(directory))
            (payload / "rg.exe").write_bytes(b"tampered")
            with self.assertRaises(ValueError):
                builder.build_compat_package(output, payload, VERSION)
            self.assertFalse((output / f"grok-zh-{VERSION}-windows-x86_64-gnu").exists())

    def test_outer_zip_overhead_cannot_exceed_legacy_download_limit(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            payload, output, native, launcher, _ = self.fixture(root)
            (payload / "rg.exe").write_bytes(random.Random(42).randbytes(16384))
            test_migration_package.MigrationPackageTests().rehash(payload)
            native.unlink()
            native = builder.native_archive_from_directory(output, payload, VERSION)
            launcher.unlink()
            bundle.append_bundle(root / "prefix.exe", native, launcher, VERSION)
            # A ZIP re-containing compressed native bytes plus the outer rg.exe
            # exceeds this synthetic legacy cap although the EXE itself fits.
            with patch.object(bundle, "MAX_ARCHIVE_BYTES", launcher.stat().st_size + 32):
                with self.assertRaisesRegex(ValueError, "transport ZIP"):
                    builder.build_compat_package(output, payload, VERSION)
            self.assertFalse((output / f"grok-zh-{VERSION}-windows-x86_64-gnu.zip").exists())

    def test_descriptor_schema_types_and_pins_are_strict(self):
        with tempfile.TemporaryDirectory() as directory:
            _, _, _, _, metadata = self.fixture(Path(directory))
            for key, value in (("schema", True), ("archive_size", True), ("archive_size", 0),
                               ("archive_size", bundle.MAX_ARCHIVE_BYTES + 1), ("archive_sha256", "F" * 64),
                               ("platform", "x86_64-pc-windows-gnu"), ("format", "unknown"), ("extra", "no")):
                with self.subTest(key=key), self.assertRaises(ValueError):
                    bundle.validate_runtime_metadata(metadata | {key: value}, VERSION)

    def test_compile_header_pins_payload_without_embedding_zip_array(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            payload, output, native, launcher, metadata = self.fixture(root)
            launcher.unlink()
            def fake_compile(command, **_):
                Path(command[-1]).write_bytes(fake_pe_prefix())
            with patch.object(builder.subprocess, "run", side_effect=fake_compile) as compile_call:
                result = builder.build_launcher(output, VERSION, native, "fixture-gcc")
            header = (output / "embedded_bootstrap_payload.h").read_text()
            self.assertIn(f'#define EMBEDDED_ARCHIVE_SIZE {metadata["archive_size"]}ULL', header)
            self.assertIn(metadata["archive_sha256"], header)
            self.assertNotIn("native_payload[]", header)
            self.assertEqual(bundle.read_footer(result)["size"], metadata["archive_size"])
            self.assertIn("-Werror", compile_call.call_args.args[0])

    def test_offline_library_has_no_network_and_retains_recovery_verbatim(self):
        script = builder.offline_validation_bytes().decode("utf-8-sig")
        for forbidden in ("New-OnlineHttpClient", "Receive-OnlineFile", "Get-ExactOnlineRelease", "https://", "HttpClient"):
            self.assertNotIn(forbidden, script)
        original = (builder.MIGRATION / "Invoke-Bootstrap.ps1").read_text(encoding="utf-8-sig")
        self.assertIn(builder.function_source(original, "Update-MigrationRuntime"), script)
        self.assertIn("$preserveRecovery = $true", script)

    def test_launcher_fast_path_and_stderr_do_not_extract_or_read_footer(self):
        text = (builder.MIGRATION / "embedded-bootstrap.c").read_text()
        version_start = text.index('if (argc == 2')
        temp_start = text.index('wchar_t temp[CAP]')
        self.assertNotIn("CreateDirectoryW", text[version_start:temp_start])
        self.assertNotIn("BCrypt", text[version_start:temp_start])
        self.assertNotIn("SHA256", text[version_start:temp_start])
        self.assertIn('diagnostics ? STD_ERROR_HANDLE : STD_OUTPUT_HANDLE', text)
        self.assertIn('windows-x64-gnu-to-msvc-embedded-v1', text)
        ps = (builder.MIGRATION / "Invoke-EmbeddedMigration.ps1").read_text(encoding="utf-8-sig")
        self.assertIn('[int]$WaitMilliseconds = 120000', ps)
        self.assertIn('Ctrl+C', ps)
        self.assertLess(ps.rindex('Test-MigrationRuntime $runtime $version'), ps.rindex('Set-MigrationReady $runtime $version'))


if __name__ == "__main__":
    unittest.main()
