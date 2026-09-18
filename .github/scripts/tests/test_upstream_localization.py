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
            "plan.notice.busy_revise", "plan.notice.busy_abandon", "plan.notice.switching_revise",
            "plan.notice.changed_on_disk", "plan.notice.switching_build", "plan.notice.revision_notes",
            "plan.notice.busy_build", "plan.notice.comment_is_command", "plan.notice.notes_are_command",
            "memory.capture.notice", "memory.capture.title.one", "memory.capture.title.many",
            "memory.capture.observation", "memory.capture.open_file", "memory.capture.attempt",
            "headless.lifecycle.memory_flush.finished", "headless.lifecycle.compact_started",
            "headless.lifecycle.compact_completed", "headless.lifecycle.compact_cancelled",
            "headless.lifecycle.auto_continue", "tool.error.read_failed", "tool.error.web_search_failed",
            "tool.error.command_failed", "tool.error.search_failed", "tool.error.edit_failed",
            "tool.error.fetch_failed", "tool.error.list_directory_failed", "btw.images.omitted_all",
            "btw.images.omitted_some", "btw.no_response", "session.usage.unsupported",
            "session.usage.invalid_response", "editor.error.invalid_command",
            "slash.command.goal.error.mid_text", "slash.command.theme.error.none_available", "session.worktree.orphaned",
        ):
            with self.subTest(key=key):
                self.assertRegex(metadata[key], r"[\u3400-\u9fff]")


    def test_literal_named_ui_keys_exist_in_a_chinese_catalog(self):
        folder = CODEGEN / "xai-grok-locale/locales"
        keys = set(load_json(folder / "zh-CN.json")) | set(load_json(folder / "zh-CN-metadata.json"))
        pattern = re.compile(r'\.named_(?:static_)?text\(\s*"([^"\n]+)"\s*,')
        missing = []
        calls = 0
        for path in sorted(CODEGEN.glob("*/src/**/*.rs")):
            text = path.read_text(encoding="utf-8")
            for match in pattern.finditer(text):
                calls += 1
                if match.group(1) not in keys:
                    line = text.count("\n", 0, match.start()) + 1
                    missing.append(f"{path.relative_to(ROOT)}:{line}: {match.group(1)}")
        self.assertGreater(calls, 500, "The scan must actually visit the shipped UI source")
        self.assertEqual(missing, [], "Unknown literal UI keys fall back to English")


    def test_literal_key_guard_runs_for_source_only_changes(self):
        workflow = (ROOT / ".github/workflows/upstream-localization.yml").read_text(encoding="utf-8")
        for event in ("pull_request", "push"):
            match = re.search(rf"^  {event}:\n(.*?)(?=^  [a-z_]+:|^permissions:)",
                              workflow, re.MULTILINE | re.DOTALL)
            self.assertIsNotNone(match, f"Missing {event} trigger")
            self.assertIn("'crates/codegen/*/src/**'", match.group(1),
                          "Literal-key checks must not depend on a simultaneous catalog edit")


    def test_shell_test_support_forwards_workspace_fixture_feature(self):
        manifest = tomllib.loads((CODEGEN / "xai-grok-shell/Cargo.toml").read_text(encoding="utf-8"))
        self.assertIn("xai-grok-workspace/test-support", manifest["features"]["test-support"])
        self.assertNotIn("test-support", manifest["features"]["default"],
                         "Test fixtures must stay opt-in for production builds")


if __name__ == "__main__":
    unittest.main()
