import importlib.util
import json
import os
from pathlib import Path
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


if __name__ == "__main__":
    unittest.main()
