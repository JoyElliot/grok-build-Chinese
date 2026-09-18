"""Verify the translated upstream material shipped by the community build."""
import json
import re
import tomllib
import unittest
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
CODEGEN = ROOT / "crates/codegen"


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"Duplicate JSON key: {key}")
        result[key] = value
    return result


def load_json(path):
    return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object)


class UpstreamLocalizationTests(unittest.TestCase):
    def test_upstream_changelogs_keep_each_entry_and_machine_field(self):
        manifest = tomllib.loads((CODEGEN / "xai-grok-shell/Cargo.toml").read_text(encoding="utf-8"))
        current = manifest["package"]["version"]
        major, minor, patch = (int(part) for part in current.split("-")[0].split("."))
        self.assertEqual((major, minor), (1, 0), "Update the reviewed changelog window for a new minor version")
        folder = CODEGEN / "xai-grok-shell/changelogs"
        for number in range(25, patch + 1):
            version = f"1.0.{number}"
            with self.subTest(version=version):
                english = load_json(folder / f"{version}.json")
                chinese = load_json(folder / f"{version}.zh-CN.json")
                self.assertEqual(len(english), len(chinese))
                for original, translated in zip(english, chinese):
                    self.assertEqual(
                        {key: value for key, value in original.items() if key != "description"},
                        {key: value for key, value in translated.items() if key != "description"},
                    )
                    self.assertRegex(translated["description"], r"[\u3400-\u9fff]")
                    self.assertNotEqual(original["description"], translated["description"])
                markdown = (folder / f"{version}.zh-CN.md").read_text(encoding="utf-8")
                # Optional community-only notes are separate from the mirrored upstream entries.
                upstream = markdown.split("## 中文社区版改进", 1)[0]
                bullets = [line[2:] for line in upstream.splitlines() if line.startswith("- ")]
                self.assertEqual(Counter(bullets), Counter(entry["description"] for entry in chinese))

    def test_public_configuration_table_has_the_same_keys(self):
        guide = CODEGEN / "xai-grok-pager/docs/user-guide"
        def keys(path):
            text = path.read_text(encoding="utf-8")
            values = re.findall(r"^\| `([^`]+)` \|", text, re.MULTILINE)
            # The policy example repeats remote_fetch outside the main table.
            return Counter(values)
        self.assertEqual(keys(guide / "26-config-reference.md"), keys(guide / "zh-CN/26-config-reference.md"))

    def test_english_guides_link_to_the_shipped_chinese_counterparts(self):
        docs = CODEGEN / "xai-grok-pager/docs"
        for section in ("user-guide", "tutorial"):
            for english in sorted((docs / section).glob("*.md")):
                chinese = english.parent / "zh-CN" / english.name
                if chinese.is_file():
                    with self.subTest(page=english.name, section=section):
                        self.assertIn(f"[简体中文](zh-CN/{english.name})", english.read_text(encoding="utf-8"))

    def test_new_owned_ui_messages_have_chinese_metadata(self):
        metadata = load_json(CODEGEN / "xai-grok-locale/locales/zh-CN-metadata.json")
        for key in (
            "prompt.flag.always_approve", "minimal.feedback.too_small",
            "slash.command.memory.error.arguments", "slash.command.flush.error.arguments",
            "slash.command.dream.error.arguments", "tasks.schedule.next_in", "tasks.schedule.due_now",
        ):
            with self.subTest(key=key):
                self.assertRegex(metadata[key], r"[\u3400-\u9fff]")


if __name__ == "__main__":
    unittest.main()
