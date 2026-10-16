#!/usr/bin/env python3
"""Reject newly resolved dependency versions/checksums outside pinned historical records."""
import json, pathlib, tomllib
ROOT=pathlib.Path(__file__).resolve().parent
allowed=json.loads((ROOT/'dependency-provenance.json').read_text())['allowed_packages']
keys={(p['name'],p['version'],p.get('source'),p.get('checksum')) for p in allowed}
resolved=tomllib.loads((ROOT/'Cargo.lock').read_text())['package']
for p in resolved:
    if p['name']=='exact-updater-selection-matrix':continue
    key=(p['name'],p['version'],p.get('source'),p.get('checksum'))
    assert key in keys,f'Unexpected dependency outside pinned historical records: {key}'
print(f'PASS: {len(resolved)-1} dependency identities/checksums remain historical')
