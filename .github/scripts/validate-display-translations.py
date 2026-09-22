#!/usr/bin/env python3
"""Validate independently versioned, exact-match display translation catalogs."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import sys
import unicodedata

spec = importlib.util.spec_from_file_location(
    "announcements", Path(__file__).with_name("validate-announcement-translations.py"))
shared = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shared)
require = shared.require
FIELDS = {
    "models": {"description": 1, "effort_label": 2, "effort_description": 2},
    "settings": {"tip": 0, "command_tag": 1, "gate_message": 0, "gate_label": 0},
    "mcp": {"connector_label": 1, "tool_label": 3, "tool_description": 3},
    "skills": {"label": 1, "description": 1},
    "marketplace": {"description": 2, "category": 2},
}
MULTILINE = {"description", "effort_description", "tool_description", "tip", "gate_message"}


def text(value, multiline=False, limit=16 * 1024):
    require(isinstance(value, str) and value.strip() and len(value.encode("utf-8")) <= limit,
            "invalid display text length")
    require(not any(unicodedata.category(c) == "Cc" and not (multiline and c == "\n")
                    for c in value), "control character in display text")


def catalog(raw, expected_version, domain):
    value = shared.document(raw, shared.MAX_CATALOG_BYTES,
                            {"schema_version", "version", "locale", "domain", "entries"})
    shared.schema(value["schema_version"])
    require(shared.version(value["version"]) == expected_version, "catalog version mismatch")
    require(value["locale"] == "zh-CN" and value["domain"] == domain, "catalog domain/locale mismatch")
    require(isinstance(value["entries"], list) and len(value["entries"]) <= 512, "invalid entries")
    seen = set()
    for entry in value["entries"]:
        require(isinstance(entry, dict), "invalid display entry")
        source_key = "source" if "source" in entry else "source_sha256"
        require(set(entry) == {"field", "context", source_key, "translation"}, "unexpected display fields")
        field, context = entry["field"], entry["context"]
        require(isinstance(field, str) and field in FIELDS[domain], "invalid display field")
        require(isinstance(context, list) and len(context) == FIELDS[domain][field], "invalid identity")
        for part in context:
            text(part, limit=1024)
        if domain == "mcp" and len(context) == 3:
            require(re.fullmatch(r"[0-9a-f]{64}", context[2]), "invalid MCP description digest")
        text(entry["translation"], field in MULTILINE)
        if source_key == "source":
            text(entry[source_key], field in MULTILINE)
            digest = hashlib.sha256(entry[source_key].encode("utf-8")).hexdigest()
        else:
            digest = entry[source_key]
            require(isinstance(digest, str) and re.fullmatch(r"[0-9a-f]{64}", digest), "invalid source digest")
        if domain == "mcp" and field == "tool_description":
            require(digest == context[2], "MCP description does not match its authoritative digest")
        key = (field, tuple(context), digest)
        require(key not in seen, "duplicate display source")
        seen.add(key)
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base")
    parser.add_argument("--domain", choices=FIELDS)
    parser.add_argument("--write-manifest", type=int, metavar="VERSION")
    args = parser.parse_args()
    require(args.write_manifest is None or args.domain, "--write-manifest requires --domain")
    root = Path(__file__).resolve().parents[2]
    for domain in ([args.domain] if args.domain else FIELDS):
        prefix = f"community/display-translations/{domain}/"
        if args.write_manifest is not None:
            number = shared.version(args.write_manifest)
            raw = (root / prefix / "catalogs" / f"{number}.json").read_bytes()
            catalog(raw, number, domain)
            manifest = dict(schema_version=1, version=number, sha256=hashlib.sha256(raw).hexdigest())
            (root / prefix / "manifest.json").write_bytes((json.dumps(manifest, indent=2) + "\n").encode())
        result = shared.validate(root, args.base, prefix, lambda raw, version: catalog(raw, version, domain))
        print(f"{domain} catalog v{result['version']} validated ({result['sha256']}).")


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, shared.subprocess.CalledProcessError) as error:
        print(f"Display catalog validation failed: {error}", file=sys.stderr)
        sys.exit(1)
