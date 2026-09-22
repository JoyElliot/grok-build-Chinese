#!/usr/bin/env python3
"""Validate the display-only catalog contract without a Rust build."""

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import unicodedata

PREFIX = "community/announcements/"
MAX_CATALOG_BYTES = 256 * 1024
FIELDS = {"title", "message", "cta_label", "cta_caption"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON key: {key}")
        result[key] = value
    return result


def document(raw, limit, keys):
    require(len(raw) <= limit, "JSON exceeds size limit")
    require(b"\r" not in raw, "catalog files must use LF line endings")
    value = json.loads(raw.decode("utf-8"), object_pairs_hook=unique_object)
    require(isinstance(value, dict) and set(value) == keys, "unexpected JSON fields")
    return value


def version(value):
    require(type(value) is int and 0 < value <= 2**64 - 1, "invalid version")
    return value


def schema(value):
    require(type(value) is int and value == 1, "unsupported schema_version")


def manifest(raw):
    value = document(raw, 4096, {"schema_version", "version", "sha256"})
    schema(value["schema_version"])
    version(value["version"])
    require(isinstance(value["sha256"], str) and
            re.fullmatch(r"[0-9a-f]{64}", value["sha256"]), "invalid sha256")
    return value


def catalog(raw, expected_version):
    value = document(raw, MAX_CATALOG_BYTES,
                     {"schema_version", "version", "locale", "entries"})
    schema(value["schema_version"])
    require(version(value["version"]) == expected_version, "catalog version mismatch")
    require(value["locale"] == "zh-CN", "unsupported locale")
    entries = value["entries"]
    require(isinstance(entries, list) and len(entries) <= 512, "invalid entries")
    seen = set()
    for entry in entries:
        require(isinstance(entry, dict) and set(entry) ==
                {"field", "source", "translation"}, "unexpected entry fields")
        field = entry["field"]
        require(isinstance(field, str) and field in FIELDS, "unsupported field")
        for name in ("source", "translation"):
            text = entry[name]
            require(isinstance(text, str) and text.strip() and
                    len(text.encode("utf-8")) <= 16 * 1024, "invalid text length")
            require(not any(unicodedata.category(c) == "Cc" and
                            not (field == "message" and c == "\n") for c in text),
                    "control character in translation")
        key = (field, entry["source"])
        require(key not in seen, "duplicate source for field")
        seen.add(key)
    return value


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], stderr=subprocess.PIPE)


def validate(root, base=None, prefix=PREFIX, validate_catalog=catalog):
    folder = root / prefix
    current = manifest((folder / "manifest.json").read_bytes())
    versions = []
    for path in (folder / "catalogs").iterdir():
        require(path.is_file() and re.fullmatch(r"[1-9][0-9]*\.json", path.name),
                f"invalid catalog filename: {path.name}")
        number = version(int(path.stem))
        raw = path.read_bytes()
        validate_catalog(raw, number)
        versions.append(number)
    require(versions and max(versions) == current["version"],
            "manifest must select the highest catalog version")
    selected = folder / "catalogs" / f"{current['version']}.json"
    require(hashlib.sha256(selected.read_bytes()).hexdigest() == current["sha256"],
            "catalog digest mismatch")
    require((folder / "catalogs/1.json").is_file(), "bundled catalog 1 must remain present")
    if base:
        # Resolve first: a missing/invalid base must not silently skip immutability.
        revision = git(root, "rev-parse", "--verify", f"{base}^{{commit}}").decode().strip()
        paths = git(root, "ls-tree", "-r", "--name-only", revision, "--", prefix)
        old_paths = paths.decode().splitlines()
        for path in old_paths:
            if re.fullmatch(re.escape(prefix) + r"catalogs/[1-9][0-9]*\.json", path):
                local = root / path
                require(local.is_file() and local.read_bytes() == git(root, "show", f"{revision}:{path}"),
                        f"published catalog is immutable: {path}")
        if prefix + "manifest.json" in old_paths:
            previous = manifest(git(root, "show", f"{revision}:{prefix}manifest.json"))
            require(current["version"] >= previous["version"], "catalog version cannot go backwards")
            if current["version"] == previous["version"]:
                require(current == previous, "changed translations require a higher version")
    return current


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", help="Git revision used to check immutable published catalogs")
    parser.add_argument("--write-manifest", type=int, metavar="VERSION",
                        help="validate a new catalog and write its version and SHA-256")
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    if args.write_manifest is not None:
        number = version(args.write_manifest)
        raw = (root / PREFIX / "catalogs" / f"{number}.json").read_bytes()
        catalog(raw, number)
        value = {"schema_version": 1, "version": number,
                 "sha256": hashlib.sha256(raw).hexdigest()}
        (root / PREFIX / "manifest.json").write_text(
            json.dumps(value, indent=2) + "\n", encoding="utf-8", newline="\n")
    value = validate(root, args.base)
    print(f"Announcement catalog v{value['version']} validated ({value['sha256']}).")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.CalledProcessError) as error:
        print(f"Announcement catalog validation failed: {error}", file=sys.stderr)
        sys.exit(1)
