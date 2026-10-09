import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

sys.dont_write_bytecode = True
SCRIPTS = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("migration", SCRIPTS / "build-windows-migration-bootstrap.py")
migration = importlib.util.module_from_spec(spec)
spec.loader.exec_module(migration)


class MigrationPackageTests(unittest.TestCase):
    def fixture(self, root):
        payload, output = root / "payload", root / "output"
        payload.mkdir()
        output.mkdir()
        for name in migration.LEGACY_FILES:
            path = payload / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(("payload " + name).encode())
        block = dict(schema=1, version="1.0.99", platform="x86_64-pc-windows-msvc", mode="executable-only",
                     manifest="SHA256SUMS.txt", executable="grok-zh.exe", installer="Install-GrokZh.ps1")
        (payload / "BUILD-INFO.txt").write_text(
            "Grok Build Windows x64 MSVC\nVersion: 1.0.99\nTarget: x86_64-pc-windows-msvc\n"
            "Profile: release-dist\nGROK-UPDATE-PROTOCOL-BEGIN\n" + json.dumps(block) +
            "\nGROK-UPDATE-PROTOCOL-END\n", encoding="utf-8")
        (payload / "future-resource.dat").write_bytes(b"only the full MSVC package needs this")
        self.rehash(payload)
        (output / "grok-zh.exe").write_bytes(b"real GNU launcher fixture")
        return payload, output

    def rehash(self, payload):
        files = sorted(p for p in payload.rglob("*") if p.is_file() and p.name != "SHA256SUMS.txt")
        (payload / "SHA256SUMS.txt").write_text("".join(
            f"{hashlib.sha256(p.read_bytes()).hexdigest()}  {p.relative_to(payload).as_posix()}\n" for p in files), encoding="utf-8")

    def test_compatibility_archive_keeps_exact_legacy_members_and_real_gnu_entry(self):
        with tempfile.TemporaryDirectory() as directory:
            payload, output = self.fixture(Path(directory))
            archive = migration.build_compat_package(output, payload, "1.0.99")
            prefix = archive.stem + "/"
            with zipfile.ZipFile(archive) as z:
                self.assertEqual(set(z.namelist()), {prefix + p for p in (*migration.LEGACY_FILES, "SHA256SUMS.txt")})
                self.assertEqual(z.read(prefix + "grok-zh.exe"), b"real GNU launcher fixture")
                info = z.read(prefix + "BUILD-INFO.txt").decode()
                self.assertIn("Target: x86_64-pc-windows-gnu", info)
                self.assertIn("Profile: GNU C launcher", info)
                block = json.loads(info.split("GROK-UPDATE-PROTOCOL-BEGIN")[1].split("GROK-UPDATE-PROTOCOL-END")[0])
                self.assertEqual(block["platform"], "x86_64-pc-windows-gnu")
                for line in z.read(prefix + "SHA256SUMS.txt").decode().splitlines():
                    digest, name = line.split("  ")
                    self.assertEqual(digest, hashlib.sha256(z.read(prefix + name)).hexdigest())
            self.assertIn(hashlib.sha256(archive.read_bytes()).hexdigest(), archive.with_suffix(".zip.sha256").read_text())

    def test_rejects_modified_payload_and_wrong_abi_before_output(self):
        for scenario in ("tamper", "wrong-abi", "wrong-version", "missing-legacy-member"):
            with self.subTest(scenario=scenario), tempfile.TemporaryDirectory() as directory:
                payload, output = self.fixture(Path(directory))
                if scenario == "tamper":
                    (payload / "grok-zh.exe").write_bytes(b"tampered")
                elif scenario == "missing-legacy-member":
                    (payload / "rg.exe").unlink()
                    self.rehash(payload)
                else:
                    info = payload / "BUILD-INFO.txt"
                    text = info.read_text(encoding="utf-8").replace(
                        "x86_64-pc-windows-msvc" if scenario == "wrong-abi" else "1.0.99",
                        "x86_64-pc-windows-gnu" if scenario == "wrong-abi" else "1.0.98")
                    info.write_text(text, encoding="utf-8")
                    self.rehash(payload)
                with self.assertRaises((ValueError, KeyError)):
                    migration.build_compat_package(output, payload, "1.0.99")
                self.assertFalse((output / "grok-zh-1.0.99-windows-x86_64-gnu").exists())

    def test_helper_pin_is_reproducible_and_exact(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            first = migration.build_helper(output, "migration-windows-x64-v1")
            data = (output / migration.ASSET).read_bytes()
            self.assertEqual(first, migration.build_helper(output, "migration-windows-x64-v1"))
            self.assertEqual(first["sha256"], hashlib.sha256(data).hexdigest())
            self.assertEqual(first["size"], len(data))
            with zipfile.ZipFile(output / migration.ASSET) as z:
                self.assertEqual(set(z.namelist()), {"MIGRATION.json", "Invoke-Migration.ps1", "SHA256SUMS.txt"})
                self.assertTrue(z.read("Invoke-Migration.ps1").startswith(b"\xef\xbb\xbf"))
            with self.assertRaises(ValueError):
                migration.build_helper(output, "latest")


if __name__ == "__main__":
    unittest.main()
