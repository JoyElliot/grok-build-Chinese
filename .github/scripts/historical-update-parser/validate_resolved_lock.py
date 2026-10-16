#!/usr/bin/env python3
"""Permit only crate versions/checksums already present in the historical lock."""
import json,pathlib,tomllib
base=pathlib.Path(__file__).resolve().parent
old=tomllib.loads((base/'historical/Cargo.lock').read_text(encoding='utf-8'))['package']
allowed={(p['name'],p['version'],p.get('source'),p.get('checksum')) for p in old}
new=tomllib.loads((base/'Cargo.lock').read_text(encoding='utf-8'))['package']
for p in new:
    if p['name']=='historical-community-parser-compatibility':continue
    key=(p['name'],p['version'],p.get('source'),p.get('checksum'))
    assert key in allowed, f'Unexpected dependency outside historical lock: {key}'
print(f'PASS: {len(new)-1} resolved dependency records use historical versions and checksums')
