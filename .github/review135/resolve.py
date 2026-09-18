"""Apply explicitly reviewed merge resolutions to a disposable checkout only."""
from __future__ import annotations
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
rules_dir = Path(sys.argv[2]).resolve()
out = Path(sys.argv[3]).resolve()
out.mkdir(parents=True, exist_ok=True)

def git(*args: str) -> str:
    return subprocess.check_output(["git", "-C", str(root), *args], text=True)

def target(name: str) -> Path:
    p = root / name
    if p.is_symlink() or not p.resolve().is_relative_to(root):
        raise ValueError(f"Unsafe review path: {name}")
    return p

pattern = re.compile(r"^<<<<<<<[^\n]*\n(.*?)^\|\|\|\|\|\|\|[^\n]*\n(.*?)^=======\n(.*?)^>>>>>>>[^\n]*\n", re.M | re.S)
files: dict = {}
for source in sorted(rules_dir.glob("rules-*.json")):
    document = json.loads(source.read_text(encoding="utf-8"))
    for name, spec in document["files"].items():
        merged = files.setdefault(name, {"conflicts": {}, "replace": []})
        for index, rule in spec.get("conflicts", {}).items():
            if index in merged["conflicts"]:
                raise ValueError(f"Duplicate resolution {name}:{index}")
            merged["conflicts"][index] = rule
        merged["replace"].extend(spec.get("replace", []))
        for key in ("copy", "delete", "append", "review_note"):
            if key in spec:
                if key in merged:
                    raise ValueError(f"Duplicate operation {name}:{key}")
                merged[key] = spec[key]

original = git("diff", "--name-only", "--diff-filter=U").splitlines()
original_blocks = []
applied = []
for name in original:
    p = target(name)
    if not p.exists():
        original_blocks.append({"path": name, "missing": True})
        continue
    text = p.read_text(encoding="utf-8")
    blocks = []
    for m in pattern.finditer(text):
        blocks.append({"line": text[:m.start()].count("\n") + 1,
                       "ours": m[1], "base": m[2], "theirs": m[3],
                       "before": "\n".join(text[:m.start()].splitlines()[-6:]),
                       "after": "\n".join(text[m.end():].splitlines()[:6])})
    original_blocks.append({"path": name, "blocks": blocks})
(out / "original-conflicts.json").write_text(json.dumps(original_blocks, ensure_ascii=False, indent=2), encoding="utf-8")

for name, spec in files.items():
    p = target(name)
    if spec.get("delete"):
        subprocess.run(["git", "-C", str(root), "rm", "--", name], check=True)
        applied.append({"path": name, "operation": "explicit-delete"})
        continue
    if spec.get("copy"):
        ref = {"ours": "50f53f06bab8309cb8299a50cd2751deb26db726",
               "theirs": "a28ee2b2063426e8816e380ccea528b9de95e5da"}[spec["copy"]]
        text = git("show", f"{ref}:{name}")
    else:
        text = p.read_text(encoding="utf-8")
    count = 0
    seen = set()
    def resolve(m: re.Match) -> str:
        global count
        index = str(count)
        count += 1
        rule = spec["conflicts"].get(index)
        if rule is None:
            return m[0]
        seen.add(index)
        if "ours_contains" in rule and rule["ours_contains"] not in m[1]:
            raise ValueError(f"Changed conflict context: {name}:{index}")
        if "text" in rule:
            value = rule["text"]
        else:
            value = m[{"ours": 1, "base": 2, "theirs": 3}[rule["choice"]]]
        applied.append({"path": name, "block": int(index), "resolution_sha256": hashlib.sha256(value.encode()).hexdigest()})
        return value
    text = pattern.sub(resolve, text)
    if seen != set(spec["conflicts"]):
        raise ValueError(f"Missing original conflicts for {name}: {set(spec['conflicts']) - seen}")
    for change in spec["replace"]:
        old, new = change["old"], change["new"]
        expected = change.get("count", 1)
        actual = text.count(old)
        if actual != expected:
            raise ValueError(f"Replacement count {name}: expected {expected}, got {actual}: {old!r}")
        text = text.replace(old, new)
    if "append" in spec:
        text += spec["append"]
    p.parent.mkdir(parents=True, exist_ok=True)
    p.write_text(text, encoding="utf-8")
    if not re.search(r"^(<<<<<<<|=======|>>>>>>>)", text, re.M):
        subprocess.run(["git", "-C", str(root), "add", "--", name], check=True)

remaining = git("diff", "--name-only", "--diff-filter=U").splitlines()
status = {"base": "50f53f06bab8309cb8299a50cd2751deb26db726",
          "upstream": "a28ee2b2063426e8816e380ccea528b9de95e5da",
          "original_conflict_paths": len(original), "remaining_conflict_paths": remaining,
          "applied": applied, "rust_typecheck_executed": False}
(out / "review-status.json").write_text(json.dumps(status, ensure_ascii=False, indent=2), encoding="utf-8")
(out / ("unresolved-merge.diff" if remaining else "resolved-content.diff")).write_text(git("diff", "HEAD", "--"), encoding="utf-8")
(out / "remaining-paths.txt").write_text("\n".join(remaining) + "\n", encoding="utf-8")
print(f"ORIGINAL_CONFLICT_PATHS={len(original)} REMAINING_CONFLICT_PATHS={len(remaining)} APPLIED_BLOCKS={len(applied)}")
print("\n".join(remaining))
control_path = rules_dir / "control.json"
control = json.loads(control_path.read_text()) if control_path.exists() else {}
selected = control.get("paths", remaining[:3])
offset = int(control.get("offset", 0))
report = []
for entry in original_blocks:
    if entry["path"] in selected:
        report.append("\nFILE: " + entry["path"])
        for i, block in enumerate(entry.get("blocks", [])):
            report.append(f"\nBLOCK {i} LINE {block['line']}\nBEFORE:\n{block['before']}\nOURS:\n{block['ours']}\nBASE:\n{block['base']}\nTHEIRS:\n{block['theirs']}\nAFTER:\n{block['after']}")
report_text = "\n".join(report)
print(f"REVIEW_EXCERPT offset={offset} total_chars={len(report_text)}")
print(report_text[offset:offset + 24000])
print("END_REVIEW_EXCERPT")
with open(os.environ["GITHUB_OUTPUT"], "a") as output:
    output.write(f"resolved={'true' if not remaining else 'false'}\n")
