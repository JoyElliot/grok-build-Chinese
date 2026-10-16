# Exact historical Windows-update parser compatibility harness

Status: The original 27-test parser/archive suite passed parent-verified Windows CI 38879857879. The newly added streamed-download/probe/activation transaction extension has NOT yet run. See TRANSACTION-COVERAGE.md for exact source coverage, seams, and remaining gaps.

## Scope and provenance

This standalone crate executes selected Rust code from JoyElliot/grok-build-Chinese release-v1.0.45, commit `3f5d07f8cb2e9be395131296a3efc4635713db7f`. The annotated tag object `7a261bf36932f6e830a7d6fda22661f9016137f5` resolves to that commit. The tag is unsigned; source identity here is established through exact Git blob hashes, not a signature claim.

- `historical/crates/codegen/xai-grok-update/src/community_release.rs`: blob `2d6fd64fa86c47afa4e58c9ba2012a498506f60f`.
- `historical/crates/codegen/xai-grok-update/src/community_release/package_protocol.rs`: blob `1bda919549c9a55a63bc19b1203f2fc0c6e6e289`.
- `historical/crates/codegen/xai-grok-product/src/lib.rs`: blob `2e93e867ef3ff903c1112af541f34f2ad2500971`.
- Full file URLs and the other snapshot hashes are in `source-provenance.json`.
- `verify_sources.py` recomputes canonical Git blob SHA-1 identities, verifies the tag's commit, and assembles `src/historical.rs` from exact line ranges. Build time repeats this check. No parser semantics are translated into Python or rewritten in Rust.
- The package-protocol and product modules are included directly by path from their unmodified complete historical files. The assembled file contains only imports, module links, comments, and byte-identical historical slices. `src/compatibility_tests.rs` supplies new test fixtures and assertions.

Historical source links:
- https://github.com/JoyElliot/grok-build-Chinese/blob/3f5d07f8cb2e9be395131296a3efc4635713db7f/crates/codegen/xai-grok-update/src/community_release.rs
- https://github.com/JoyElliot/grok-build-Chinese/blob/3f5d07f8cb2e9be395131296a3efc4635713db7f/crates/codegen/xai-grok-update/src/community_release/package_protocol.rs

Exact functions under test include `PackageProtocol::parse`, `PackageProtocol::check_manifest`, `safe_path`, `parse_manifest`, `read_zip_metadata`, `canonical_release_version`, `parse_release_version`, `release_asset_name_for`, `parse_sha256_digest`, `select_asset_for_platform`, `validate_zip_layout`, `hash_manifest_entry`, and `extract_verified_windows_executable`.

## Native payload and truthful transport metadata

`prepare_native_fixture.py` requires the verified CI native package. It pins the EXE SHA-256 to `8626e60c908f44848190adc9f3547241c651ed8179601d93e33b91469dc76f20` (141,354,496 bytes), source commit `ddede0f98fbbef6349b5040b8390afb9f4d3a5cd`, and the existing human-readable `Target: x86_64-pc-windows-msvc` statement.

Input provenance supplied with the verified artifact:
- Workflow run 38842814822, attempt 1, artifact 12022879975
- Artifact ZIP SHA-256 `4a3d80bac40ba3c63bfeb00f4b97976f6727842463a67c10d7d4b15749278f33`
- Native package ZIP SHA-256 `802fbd20fee858fc90521438c23d5d9934370c5a97815680c208305e19a2b41d`

The isolated fixture keeps every native payload file unchanged. It changes only `BUILD-INFO.txt` and regenerated `SHA256SUMS.txt`, and uses a GNU-named archive/root for the historical transport selector. Its protocol says:

- `platform`: `x86_64-pc-windows-gnu` (explicitly described as compatibility transport)
- `actual_runtime_target`: `x86_64-pc-windows-msvc`
- Human-readable `Target`: remains `x86_64-pc-windows-msvc`
- `compatibility_transport`: explains the legacy selector and actual native payload

It is prominently labeled an unpublished parser test fixture. It is not a product artifact, migration wrapper, release, or proposed installer package. No wrapper binary is introduced. The original parser-only tests do not execute EXEs. The transaction extension executes genuine historical/native --version probes and replaces only an isolated genuine-old-EXE copy, under a credential-free test environment. No artifact is uploaded by these scripts. Any surrounding workflow should keep generated fixtures out of uploaded product artifacts.

The positive native test uses the full fixture, verifies its local outer SHA-256, calls the exact historical archive extraction function, and checks the isolated extracted EXE is byte-identical to the native payload. It never calls or launches the installer. Python is used only for snapshot provenance, fixture construction, and result accounting.

## Tests prepared

The original suite has 24 compatibility test functions (one is Windows-MSVC-only), plus original product/package tests. The extension adds nine Windows transaction tests and the four unchanged cancellation tests. See TRANSACTION-COVERAGE.md.

Coverage includes a truthful extra runtime-target field; native-payload full archive extraction; synthetic inert archive extraction; negative protocol platform, version, schema, type, mode, manifest and entry-point names; malformed markers, UTF-8 and size limits; invalid/duplicate/missing manifest entries; malformed GitHub digest; incorrect asset name or URL; mutable/draft release, incorrect prerelease flag, duplicate/non-uploaded/zero-size assets; inner payload-hash mismatch with candidate cleanup; wrong root, extra file, and existing-destination protection.

A deliberate limitation test demonstrates that a syntactically valid but wrong 64-digit outer hash passes metadata selection alone. The real network download function must compare bytes. The native fixture test independently checks its local outer hash, but does not execute `download_verified` or authenticate a live GitHub response.

## Windows invocation

Python 3.11 or newer is required for the standard-library TOML provenance checks. Set `GROK_DIRECT_NATIVE_EXE` to the extracted verified native `grok-zh.exe`, with its original package companion files in the same directory. Also provide `GROK_DIRECT_HISTORICAL_EXE`, `GROK_DIRECT_HISTORICAL_ARCHIVE`, and `GROK_DIRECT_NATIVE_ARCHIVE` as documented in TRANSACTION-COVERAGE.md. Then run:

`pwsh -NoProfile -File .github/scripts/historical-update-parser/Run-HistoricalParser.ps1`

Or pass `-NativeExe C:\path\to\grok-zh.exe` explicitly. The launcher changes into its own directory. Its own `rust-toolchain.toml` pins Rust 1.94.0 with a minimal, host-only profile; it does not inherit Linux cross-compilation targets from the application workspace. Only the small standalone parser crate is compiled. No full Grok rebuild, release workflow, branch operation, or dispatch occurs here.

The launcher validates source provenance, generates the fixture, resolves dependencies from the historical lock seed, verifies that every resolved crate version/checksum exists in the historical lock, retains the resulting lock and Cargo metadata, then builds using `cargo test --locked --no-run --message-format=json` and runs the test EXE directly under the isolated environment documented in TRANSACTION-COVERAGE.md. All direct dependency versions in Cargo.toml are pinned exactly from the historical lock, including the small async, progress, cancellation, and Windows helper dependencies used by the transaction extension.

The refreshed checked-in seed contains the exact 174-record transitive dependency closure from the historical lock for the parser and transaction helpers. Cargo may prune unused records after feature resolution. That resolution has NOT been verified locally. If Cargo resolves a version/checksum outside the historical lock, validation stops before tests. No test pass is reported unless the real native-payload test and Cargo test summaries are present and successful.

Results are written under the harness's `results/` directory:
- `source-provenance-check.json`: source checks from preparation
- `native-fixture-provenance.json`: input EXE hash/runtime target, package hashes, local fixture identity
- `resolved-Cargo.lock` and `resolved-cargo-metadata.json`: actual CI resolution
- `cargo-metadata.log`, `rust-tests.log`, `launcher-error.log`: available diagnostics
- `rust-execution-status.json`: actual exit status, test summaries, positive native-case status and EXE hash

Generated ZIPs and test outputs are ignored by `.gitignore`. Do not publish or upload the generated ZIP as a product. Results/logs may be retained for review.

## What a future pass would and would not establish

A pass would establish that the pinned release-v1.0.45 parser accepts this precisely defined local GNU compatibility transport carrying unchanged native MSVC EXE bytes and verifies inner hashes. It would also show that unknown JSON fields can truthfully document actual runtime ABI without breaking this old parser. The parser does not itself understand or enforce that extension, inspect compiler provenance, or validate ABI; CI/build evidence supplies the runtime claim.

A parser-only pass would NOT establish activation; the transaction extension separately exercises isolated Windows replacement and probes. Neither establishes a complete live update chain, live GitHub metadata authenticity or production asset visibility, network/certificate/redirect policy, a running old process locking its mapped EXE, installer compatibility, AV verdict, or execution of the installed native binary’s self-update command. The extension covers original update-lock contention, an explicit file-sharing model, and copy-failure rollback; those results must not be generalized to every Windows failure condition. The native replacement's updater must independently map its actual MSVC target to supported current policy. In this historical source, `current_community_platform` only accepts Windows GNU; compiling it for native Windows MSVC returns an error. The conditional Windows test makes that limitation explicit.

## v1.0.8 bridge limitation

The published `v1.0.8` release itself has two immutable GNU Windows assets: ZIP and `.sha256`. Its tag `cc307a9c22f2ae19e1e48d1568dd6ff8b9394b45` resolves to commit `dfcf92b37854f5b2c0dacc66a9e4195f3881dcae`. Its updater source is saved verbatim in `bridge-source-v1.0.8/community_release.rs`, blob `d914fae2c4475322197dafa8da85f989741d801b`.

That is different from what a client already running v1.0.8 requires when selecting a later modern release: `select_asset_for_platform` at lines 397-408 compares the complete uploaded asset list to `expected_release_asset_names`. Later versions require exactly six assets: Windows GNU ZIP + sidecar, macOS archive + sidecar, Linux GNU archive + sidecar. A release containing only the single GNU compatibility ZIP is rejected by that v1.0.8 selector before archive download. Adding an extra native-MSVC-named asset also changes the exact set and does not by itself solve this constraint.

The older v1.0.5 clients ignore the modern `release-v` namespace and need the frozen plain `v1.0.8` bridge. The 1.0.45 parser's selected-platform-only trust policy is more permissive than the v1.0.8 exact-set rule. Therefore a 1.0.45 pass cannot be generalized to every older installed client or used to justify retiring bridge assets. The integrated selection matrix includes exact v1.0.8 selection source with synthetic metadata; its Windows compilation and execution are pending this extension run. It does not run a genuine v1.0.8 executable or prove live bridge availability. No immutable release assets were modified.

Source: https://github.com/JoyElliot/grok-build-Chinese/blob/dfcf92b37854f5b2c0dacc66a9e4195f3881dcae/crates/codegen/xai-grok-update/src/community_release.rs#L397-L408
