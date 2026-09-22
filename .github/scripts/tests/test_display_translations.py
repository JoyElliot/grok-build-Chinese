import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "display_validator", Path(__file__).resolve().parents[1] / "validate-display-translations.py")
validator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(validator)


class DisplayCatalogTests(unittest.TestCase):
    def document(self, entries, domain="models", version=1):
        return (json.dumps(dict(schema_version=1, version=version, locale="zh-CN",
                                domain=domain, entries=entries), ensure_ascii=False) + "\n").encode()

    def entry(self):
        return dict(field="description", context=["grok-future"],
                    source="New official copy", translation="新官方文案")

    def test_identity_and_source_are_both_part_of_key(self):
        first = self.entry()
        entries = [first, {**first, "context": ["grok-other"]},
                   {**first, "source": "Changed official copy"}]
        validator.catalog(self.document(entries), 1, "models")
        hashed = {key: value for key, value in first.items() if key != "source"}
        hashed["source_sha256"] = hashlib.sha256(first["source"].encode()).hexdigest()
        with self.assertRaisesRegex(ValueError, "duplicate"):
            validator.catalog(self.document([first, hashed]), 1, "models")

    def test_domain_fields_identity_and_control_bytes_are_strict(self):
        first = self.entry()
        bad_entries = [dict(first, field="api_url"), dict(first, context=[]),
                       dict(first, context=["model\nname"]), dict(first, translation="\x1b[2J"),
                       dict(first, source_sha256="0" * 64), dict(first, source=None)]
        for entry in bad_entries:
            with self.subTest(entry=entry), self.assertRaises(ValueError):
                validator.catalog(self.document([entry]), 1, "models")
        with self.assertRaisesRegex(ValueError, "domain"):
            validator.catalog(self.document([first]), 1, "skills")
        with self.assertRaisesRegex(ValueError, "invalid entries"):
            validator.catalog(self.document([first] * 513), 1, "models")

    def test_mcp_description_hash_binds_label_and_body_to_same_identity(self):
        source = "Complete official tool description"
        digest = hashlib.sha256(source.encode()).hexdigest()
        entry = dict(field="tool_description", context=["future", "search", digest],
                     source_sha256=digest, translation="官方完整说明")
        validator.catalog(self.document([entry], "mcp"), 1, "mcp")
        for changed in ("invalid", "0" * 64):
            invalid = copy.deepcopy(entry)
            invalid["context"][2] = changed
            with self.assertRaises(ValueError):
                validator.catalog(self.document([invalid], "mcp"), 1, "mcp")

    def test_independent_versions_immutable_history_and_empty_replacement(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            def git(*args):
                return subprocess.check_output(["git", "-C", directory, *args],
                                               stderr=subprocess.PIPE).decode().strip()
            def write(domain, version, entries):
                prefix = f"community/display-translations/{domain}/"
                folder = root / prefix
                (folder / "catalogs").mkdir(parents=True, exist_ok=True)
                raw = self.document(entries, domain, version)
                (folder / f"catalogs/{version}.json").write_bytes(raw)
                (folder / "manifest.json").write_bytes(json.dumps(dict(
                    schema_version=1, version=version, sha256=hashlib.sha256(raw).hexdigest())).encode())
                return prefix
            models = write("models", 1, [self.entry()])
            skills = write("skills", 1, [])
            git("init", "--quiet")
            git("-c", "core.autocrlf=false", "add", "community")
            git("-c", "user.name=Catalog Test", "-c", "user.email=test@example.invalid",
                "-c", "core.hooksPath=/dev/null", "-c", "commit.gpgSign=false",
                "commit", "--quiet", "-m", "catalog baseline")
            base = git("rev-parse", "HEAD")
            write("models", 2, [])
            for domain, prefix, version in (("models", models, 2), ("skills", skills, 1)):
                result = validator.shared.validate(root, base, prefix,
                    lambda raw, number: validator.catalog(raw, number, domain))
                self.assertEqual(result["version"], version)
            write("skills", 1, [dict(field="description", context=["future"],
                                    source="New copy", translation="新文案")])
            with self.assertRaisesRegex(ValueError, "immutable"):
                validator.shared.validate(root, base, skills,
                    lambda raw, number: validator.catalog(raw, number, "skills"))


if __name__ == "__main__":
    unittest.main()
