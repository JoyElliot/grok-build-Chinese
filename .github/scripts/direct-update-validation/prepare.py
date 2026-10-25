#!/usr/bin/env python3
"""Generate exact-source test copies, with provenance hashes, from local Git."""
import hashlib, json, pathlib
BASE = pathlib.Path(__file__).resolve().parent
ROOT = BASE.parents[2]
HISTORICAL = "3a1359d201957ccd0e33e24684dd4b291a71971b"
PREFIX = ".github/scripts/historical-update-parser/"
OUT = BASE / "generated"
checks = []
def write(name, data, source):
    path = OUT / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    checks.append({"file": name, "source": source, "sha256": hashlib.sha256(data).hexdigest()})
PINNED = {
    PREFIX + "historical/crates/codegen/xai-grok-update/src/community_release.rs": "2d6fd64fa86c47afa4e58c9ba2012a498506f60f",
    PREFIX + "historical/crates/codegen/xai-grok-update/src/community_release/package_protocol.rs": "1bda919549c9a55a63bc19b1203f2fc0c6e6e289",
    PREFIX + "bridge-source-v1.0.8/community_release.rs": "d914fae2c4475322197dafa8da85f989741d801b",
}
def blob(path):
    local = {
        PREFIX + "historical/crates/codegen/xai-grok-update/src/community_release.rs": "v1_0_45/community_release.rs",
        PREFIX + "historical/crates/codegen/xai-grok-update/src/community_release/package_protocol.rs": "v1_0_45/package_protocol.rs",
        PREFIX + "bridge-source-v1.0.8/community_release.rs": "v1_0_8/community_release.rs",
    }[path]
    data = (BASE / "historical" / local).read_bytes()
    digest = hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest()
    if digest != PINNED[path]:
        raise RuntimeError(f"Historical source blob mismatch: {path}")
    return data
source = ROOT / "crates/codegen/xai-grok-update/src"
for path in [source / "community_release.rs", *sorted((source / "community_release").glob("*.rs"))]:
    relative = path.relative_to(source)
    write("current/" + str(relative), path.read_bytes(), str(path.relative_to(ROOT)))
write("community_update_cancel.rs", (source / "community_update_cancel.rs").read_bytes(), "current cancellation implementation")
for revision, path in {
    "v1_0_45": "historical/crates/codegen/xai-grok-update/src/community_release.rs",
    "v1_0_8": "bridge-source-v1.0.8/community_release.rs",
}.items():
    write(f"{revision}/community_release.rs", blob(PREFIX + path), f"{HISTORICAL}:{PREFIX + path}")
write("v1_0_45/community_release/package_protocol.rs", blob(PREFIX + "historical/crates/codegen/xai-grok-update/src/community_release/package_protocol.rs"), "pinned v1.0.45 protocol")
OLD_MSVC_COMMIT = "74c5269942d9a47bf09a9268db0bb5c145967e27"
OLD_MSVC_PINS = {'community_release.rs': '78231c980b68b6745359db9cc619170c5fbcf699', 'community_release/package_protocol.rs': 'cf843bb275223f9a129b20e24ec37df5b50041a0', 'community_release/embedded_runtime.rs': '81acf557da7009dfc1856da14f73d901f87ff6d4', 'community_release/embedded_runtime_tests.rs': '4a6c5ff10068fbadc310527b6b368a9b4f105ea5'}
for relative, expected in OLD_MSVC_PINS.items():
    data = (BASE / "historical/old_msvc" / relative).read_bytes()
    if hashlib.sha1(f"blob {len(data)}\0".encode() + data).hexdigest() != expected:
        raise RuntimeError(f"Old MSVC source blob mismatch: {relative}")
    write("old_msvc/" + relative, data, f"{OLD_MSVC_COMMIT}:crates/codegen/xai-grok-update/src/{relative}")
# #[path] modules resolve child modules beside their named source file.
for revision in ["current", "v1_0_45", "old_msvc"]:
    for child in (OUT / revision / "community_release").glob("*.rs"):
        (OUT / revision / child.name).write_bytes(child.read_bytes())
for revision in ["current", "v1_0_45", "v1_0_8", "old_msvc"]:
    path = OUT / revision / "community_release.rs"
    # Add a separate test module; every byte of the production source remains
    # unchanged, and the source hash above excludes this explicit adapter.
    with path.open("ab") as stream:
        stream.write(f"\n#[cfg(test)]\nconst HARNESS_EXACT_ASSET_SET: bool = {str(revision == 'v1_0_8').lower()};\n".encode())
        stream.write(b'\n#[cfg(test)]\n#[path = "../../package_compatibility.rs"]\nmod direct_compatibility;\n')
with (OUT / "old_msvc/community_release.rs").open("ab") as stream:
    stream.write(b'\n#[cfg(test)]\n#[path = "../../old_msvc_compatibility.rs"]\nmod old_msvc_compatibility;\n')
(BASE / "results").mkdir(exist_ok=True)
(BASE / "results/source-provenance.json").write_text(json.dumps({"historical_commit": HISTORICAL, "sources": checks}, indent=2) + "\n")
print(f"Verified/generated {len(checks)} exact source copies")
