# Exact-source updater selection matrix

Prepared read-only from the repository's full Git history, in a separate workspace. No production updater, packaging policy, frozen bridge, release, or remote state was modified.

## What this answers

The historical parser passing on native bytes in GNU transport does **not** establish end-to-end update continuity. Three independent metadata policies apply:

1. **v1.0.8** accepts a later modern release only if its complete uploaded asset set is exactly the historical six names: GNU Windows archive + sidecar, Apple Silicon archive + sidecar, Linux x64 GNU archive + sidecar. Adding a native MSVC name, adding other platforms, dropping a sidecar, or malformed unrelated metadata makes it incompatible. With a frozen six-asset `release-v1.0.45` fixture retained, it selects that bridge instead of the newer incompatible fixture.
2. **v1.0.45** validates only its selected platform archive. It can select a newer GNU-named archive even when sidecars are absent and unrelated native/platform assets exist. A native-MSVC-only release is not selectable by its GNU Windows client. Retaining compatible historical bridge and transport releases remains essential.
3. **Current MSVC (`ddede0f`)** prefers a same-version native `-msvc.zip`. If absent, it selects the newest potential GNU-named package. This is metadata eligibility only. If a native candidate is present but invalid, it fails closed instead of falling back to GNU or an older release. Newer GNU metadata likewise takes priority over an older native asset, and invalid newest metadata is not silently hidden.

**Direct native EXE in GNU transport is currently UNSUPPORTED for the next update by the MSVC client.** The existing extraction path routes GNU fallback through `embedded_runtime::extract_msvc_executable`, which invokes unchanged `extract_windows_package(..., require_embedded=true)`. Without `embedded_runtime`, it returns `GNU release is not a recognized embedded MSVC package; use its full installer for a GNU downgrade`. `actual_runtime_target` is an unknown optional JSON field, not a routing instruction. The bundle now includes unchanged current extraction source and four extraction assertions: an inert direct-GNU rejection, an inert native-name extraction control, the required real-native-GNU rejection, and an authentic native-ZIP positive control extracting the exact same pinned native EXE. None has run locally; the conclusion is source-inspected until CI executes them. No production fix or protocol rewrite is proposed by this artifact.

## Exact provenance

- v1.0.8: [`dfcf92b37854f5b2c0dacc66a9e4195f3881dcae`](https://github.com/JoyElliot/grok-build-Chinese/blob/dfcf92b37854f5b2c0dacc66a9e4195f3881dcae/crates/codegen/xai-grok-update/src/community_release.rs#L397-L438)
- v1.0.45: [`3f5d07f8cb2e9be395131296a3efc4635713db7f`](https://github.com/JoyElliot/grok-build-Chinese/blob/3f5d07f8cb2e9be395131296a3efc4635713db7f/crates/codegen/xai-grok-update/src/community_release.rs#L379-L441)
- Current MSVC selection: [`ddede0f98fbbef6349b5040b8390afb9f4d3a5cd`](https://github.com/JoyElliot/grok-build-Chinese/blob/ddede0f98fbbef6349b5040b8390afb9f4d3a5cd/crates/codegen/xai-grok-update/src/community_release.rs#L452-L596)
- Current GNU fallback extraction routing: [L1090–1097](https://github.com/JoyElliot/grok-build-Chinese/blob/ddede0f98fbbef6349b5040b8390afb9f4d3a5cd/crates/codegen/xai-grok-update/src/community_release.rs#L1090-L1097)
- Required embedded descriptor: [caller L24–31](https://github.com/JoyElliot/grok-build-Chinese/blob/ddede0f98fbbef6349b5040b8390afb9f4d3a5cd/crates/codegen/xai-grok-update/src/community_release/embedded_runtime.rs#L24-L31), [rejection L1131–1134](https://github.com/JoyElliot/grok-build-Chinese/blob/ddede0f98fbbef6349b5040b8390afb9f4d3a5cd/crates/codegen/xai-grok-update/src/community_release.rs#L1131-L1134)

`source-provenance.json` pins commits, Git blob SHA-1, SHA-256, byte sizes, source URLs, and inclusive line ranges. `sources/` contains eight byte-identical Git blobs. The generator emits unchanged source slices without substitutions. Only narrow harness adapters and synthetic fixture generation are authored. The full current extraction/protocol snapshots are compiled unchanged. Current ZIP extraction helpers are emitted as verbatim ranges alongside the exact selection functions; the complete embedded-runtime module is present only because the unchanged top-level extraction function calls it. Tests do not create or execute a launcher.

`bootstrap_sources.py --repo <local-checkout>` imports pinned blobs using read-only `git show`/`rev-parse`. It is a one-time reconstruction tool, not part of CI or a fetch operation.

## Synthetic fixture boundary

All 171 cases in `fixtures/selection-matrix.json` are explicitly **SYNTHETIC_OFFLINE_API_METADATA**. The digests, sizes, and release objects are test inputs. They are not copied from GitHub and do not prove live immutable inventory, real asset hashes, download behavior, or frozen bridge availability. The six-asset `release-v1.0.45` fixture models the retained bridge contract; source code is not used as evidence of a live release's inventory. In particular, the current source's generic `>=1.0.36` expected-assets test helper cannot establish the inventory of frozen historical release-v1.0.45.

Cases cover the retained bridge chain, exact-set versus selected-asset policies, all seven current platforms, native preference, GNU fallback discovery, invalid newest metadata fail-closed behavior, duplicates, wrong URL/digest/state/size, namespace/immutable/channel rules, asset ordering, and requesting ABI preservation. Metadata matrix cases perform no network or filesystem mutation. The four separate extraction cases create and remove private temporary files only; the real-payload rejection checks the existing pinned fixture without executing its EXE.

## Status and execution

Local static preflight passed:

- Eight Git blob hashes/SHA-256/size checks
- Three unchanged generated source modules, including the exact current extraction dependency closure
- 171 deterministic metadata cases and generated test names, plus four authored extraction assertions
- Historical-only dependency identity/checksum check
- Python syntax and shell syntax checks

**Rust compilation and matrix execution have NOT run locally: neither `rustc` nor `cargo` is installed.** `results/selection-execution-report.json` reports `NOT_EXECUTED` (0/171 metadata and 0/4 extraction assertions), not a test pass. The report always separates assertion-suite outcome from product continuity: `current_native_direct_transport_supported` is `false` for the pinned direct-GNU-transport scope even when expected-rejection tests pass.

The real-native rejection assertion requires the existing parser harness fixture `fixtures/native-release.json` and `fixtures/grok-zh-1.0.103-windows-x86_64-gnu.zip`, produced by its `prepare_native_fixture.py`. Nested integration finds these through `CARGO_MANIFEST_DIR`; standalone runs must set `GROK_DIRECT_NATIVE_FIXTURE_DIR` to that fixture directory. Missing fixtures fail rather than silently skip. The test hashes both the outer ZIP and its unchanged EXE (SHA-256 `8626e60c908f44848190adc9f3547241c651ed8179601d93e33b91469dc76f20`), then invokes the exact top-level current extraction function and asserts the specific rejection with no candidate/temp file left behind.

The authentic native-route positive control additionally requires `GROK_DIRECT_NATIVE_ARCHIVE` to point to the already available original `direct-results/outer/grok-zh-1.0.103-windows-x86_64-msvc.zip`. It first verifies fixed outer SHA-256 `802fbd20fee858fc90521438c23d5d9934370c5a97815680c208305e19a2b41d`, selects native-named metadata, and invokes the same unchanged current extractor. The extracted EXE must equal the same pinned native EXE hash. This distinguishes the GNU missing-descriptor gap from a broken extraction harness; it does not exercise a second distinct-version update. No new payload is needed and no executable is run.

On a Rust-equipped host, use `bash run.sh` or `pwsh -File Run-SelectionMatrix.ps1`. These build only the isolated selection crate, not the product. The copied historical lock is a seed, not claimed Cargo-resolved; `cargo metadata` may prune optional feature records, then dependency identities/checksums must remain within `dependency-provenance.json`. Tests run with `--locked` after that check. Toolchain is Rust 1.94.0.

## Preferred integration with the existing parser harness

No second dependency graph is needed: the seven direct/test dependencies (`anyhow 1.0.104`, `semver 1.0.28`, `serde 1.0.228`, `serde_json 1.0.149`, `sha2 0.10.9`, `zip 3.0.0`, `tempfile 3.27.0`) already exist in the historical parser harness.

1. Copy this self-contained directory under `historical-update-parser/selection-matrix/`.
2. In that crate's `src/lib.rs`, add:

   ```rust
   #[cfg(test)]
   #[path = "../selection-matrix/src/lib.rs"]
   mod selection_matrix;
   ```

3. Have its build/provenance step run `python selection-matrix/verify_sources.py --check`. Keep that invocation independent of the parent generator. Add rerun tracking for `selection-matrix`.
4. Existing `cargo test --locked -- --nocapture` runs the nested matrix cases without using this directory's Cargo manifest/lock/toolchain. The modules use parent-relative paths and remain valid standalone or nested.
5. Report nested results using `python selection-matrix/write_execution_report.py --exit-code <cargo-exit-code> --stage "integrated exact-source selection matrix" --log ../results/rust-tests.log`. The report accepts both standalone and nested Rust test names and requires every named metadata assertion and all four extraction assertions to appear as `ok`. The authored extraction names are `selection_matrix::current_msvc::extraction_tests::{authentic_native_msvc_archive_extracts_identical_native_exe,direct_gnu_transport_without_embedded_runtime_is_rejected,native_named_package_extracts_declared_inert_bytes,real_native_gnu_transport_is_rejected_for_current_msvc}`.

The parser harness owner must agree before integration edits. This bundle does not change their files.
