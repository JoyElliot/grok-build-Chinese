import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location(
    "msvc_input", Path(__file__).resolve().parents[1] / "msvc-build-input.py"
)
msvc_input = importlib.util.module_from_spec(spec)
spec.loader.exec_module(msvc_input)


class MsvcBuildInputTests(unittest.TestCase):
    def test_native_consumer_rejects_wrong_identity_or_bytes_before_staging(self):
        env = dict(GITHUB_SHA="a" * 40, GITHUB_RUN_ID="123", GITHUB_RUN_ATTEMPT="1", GROK_VERSION="1.0.99")
        for changed in (None, "commit", "run_id", "run_attempt", "version", "target", "build_host",
                        "profile", "features", "size", "sha256", "bytes", "extra-member"):
            with self.subTest(changed=changed), tempfile.TemporaryDirectory() as tmp, patch.dict(os.environ, env):
                root = Path(tmp)
                source = root / "source.exe"
                source.write_bytes(b"verified MSVC test payload")
                directory, destination = root / "input", root / "native" / "grok-zh.exe"
                msvc_input.write_input(source, directory)
                manifest_path = directory / "build-input.json"
                manifest = json.loads(manifest_path.read_text())
                if changed == "bytes":
                    (directory / "grok-zh.exe").write_bytes(b"different payload")
                elif changed == "extra-member":
                    (directory / "unexpected.dll").write_bytes(b"extra")
                elif changed:
                    manifest[changed] = 999 if changed == "size" else "wrong"
                    manifest_path.write_text(json.dumps(manifest))
                if changed:
                    with self.assertRaises(ValueError):
                        msvc_input.verify_input(directory, destination)
                    self.assertFalse(destination.exists())
                else:
                    msvc_input.verify_input(directory, destination)
                    self.assertEqual(destination.read_bytes(), source.read_bytes())


ROOT = Path(__file__).resolve().parents[3]
PROBE = ROOT / ".github/scripts/tests/Test-MsvcToolWrappers.ps1"
LOCK_ENTRY = '[[package]]\nname = "{0}"\nversion = "{1}"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n'


class MsvcCargoProbeLockTests(unittest.TestCase):
    """The cc/blake3 probe must follow the lockfile being built (PR merge refs included)."""

    def test_probe_versions_come_from_the_product_lockfile(self):
        source = PROBE.read_text(encoding="utf-8")
        self.assertNotRegex(source, r'(cc|find-msvc-tools|blake3) = "=\d')
        self.assertIn("$LockFile = (Join-Path $PSScriptRoot '../../../Cargo.lock')", source)
        for name in ("cc", "find-msvc-tools", "blake3"):
            self.assertIn(f"$($locked['{name}'])", source)
        # Missing or ambiguous entries must stop the probe instead of skipping it.
        self.assertIn("throw \"Product Cargo.lock not found", source)
        self.assertIn("must lock exactly one crates.io version", source)
        self.assertLess(source.index("Get-LockedRegistryVersion $lock $name"),
                        source.index("cargo generate-lockfile --offline"))
        lock = (ROOT / "Cargo.lock").read_text(encoding="utf-8")
        for name in ("cc", "find-msvc-tools", "blake3"):
            with self.subTest(name=name):
                self.assertEqual(len(re.findall(rf'^name = "{re.escape(name)}"\nversion = ', lock, re.M)), 1)

    @unittest.skipUnless(shutil.which("pwsh"), "pwsh not installed")
    def test_probe_resolves_lockfile_or_fails_loudly(self):
        env = {k: v for k, v in os.environ.items() if k != "CC_x86_64_pc_windows_msvc"}

        def run(lock, tmp):
            result = subprocess.run(
                [shutil.which("pwsh"), "-NoProfile", "-File", str(PROBE), "-WrapperDirectory", tmp,
                 "-CargoProbe", "-LockFile", str(lock)],
                env=env, capture_output=True, text=True, encoding="utf-8", errors="replace")
            return result.returncode, " ".join((result.stdout + result.stderr).split())

        with tempfile.TemporaryDirectory() as tmp:
            good = Path(tmp) / "good.lock"
            good.write_text("".join(LOCK_ENTRY.format(n, v) for n, v in (
                ("cc", "1.2.48"), ("find-msvc-tools", "0.1.14"), ("blake3", "1.8.2"))), encoding="utf-8")
            code, output = run(good, tmp)
            # Versions resolved; the probe then stops at the (absent) target cl.exe wrapper.
            self.assertNotEqual(code, 0)
            self.assertIn("BLAKE3 cross builds require", output)

            missing_cc = Path(tmp) / "missing.lock"
            missing_cc.write_text(LOCK_ENTRY.format("blake3", "1.8.2"), encoding="utf-8")
            code, output = run(missing_cc, tmp)
            self.assertNotEqual(code, 0)
            self.assertIn("exactly one crates.io version of cc", output)

            code, output = run(Path(tmp) / "absent.lock", tmp)
            self.assertNotEqual(code, 0)
            self.assertIn("Product Cargo.lock not found", output)


if __name__ == "__main__":
    unittest.main()
