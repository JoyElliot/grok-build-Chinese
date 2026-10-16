# Historical updater transaction extension

Previous baseline: parent-verified Windows CI 38879857879 passed the original 27 parser/archive tests. The extension described here is newly prepared; it is NOT covered by that earlier result. Local Rust execution is unavailable. All new Rust execution remains pending Windows CI.

## Exact source coverage and seams

Historical revision is `3f5d07f8cb2e9be395131296a3efc4635713db7f` (release-v1.0.45). New complete source snapshots are hash-verified:

- auto_update.rs: Git blob ff5024f2778a79148cddb047a4363059a471b872
- community_update_cancel.rs: e412e2d476a4e017a01132b1f314c06ddb31ddee
- xai-tty-utils/src/lib.rs: 4fe44241a5cdcc874d34a0b41f86a2c5fd052485
- xai-grok-tools/src/util/spawn.rs: 12d0022e5cafa4e8f8c8d66bb9aeed5f04efe067 (proves the historical re-export used by probe_version)

`verify_sources.py` generates exact original function bodies, preserving their bytes. `build.rs` rejects any mismatch. Added imports, namespace wiring, test assertions, and the two intentional seams are visibly separated from copied source.

1. Discovery/selection: unchanged `select_latest_compatible_release`, `parse_release_version`, and `select_asset_for_platform`. Uses actual captured GitHub metadata for release-v1.0.45, plus explicitly synthetic unpublished metadata for the proposed native payload. Tests newest compatible selection and rejection of an ineligible higher draft. Network fetch, pagination, and runtime platform selection remain outside this fixture seam.
2. Download: unchanged `write_verified_download`, including create_new, bounded chunk accumulation, declared-size enforcement, SHA-256 comparison, flush/sync, cancellation checks, and partial-file cleanup. Real old and native archive bytes are streamed in bounded 64-KiB chunks from local files. The outer `download_verified` HTTP adapter is NOT called; headers, status, TLS, allowed-host policy, redirect handling and live API authenticity are not tested here.
3. Extraction: unchanged `extract_verified_executable` dispatch and ZIP verification pipeline, including inner manifest, path/layout checks, hashes, and candidate cleanup. Unix helpers are included solely to compile the unchanged dispatch; these new transaction tests run on Windows.
4. Candidate probe: unchanged `probe_version`, `smoke_test_binary`, `community_version_output_matches`, 30-second timeout constant, and actual historical `detach_command` function. Runs genuine candidate bytes, not a fixture executable. No version-stub script or binary is substituted.
5. Windows transaction: unchanged `install_community_release` slices L1161–1171 (filename guard), L1173–1174 (temporary sibling paths), and L1177–1231 (extract/spawn_blocking cleanup, candidate probe/version gate, cancellation activation boundary, Windows update lock/replacement, candidate cleanup). The destination parameter supplies an isolated copy of the genuine historical EXE. Original L1175's HTTP adapter call is replaced explicitly by the bounded local file stream passed to unchanged `write_verified_download`.
6. Activation helpers: unchanged `acquire_windows_update_lock`, `windows_replace_exe_locked`, `windows_replace_exe`, `sweep_old_exe_backups`, and `unique_temp_sibling`. Tests normal replacement, lock contention, a sharing-handle-locked genuine old file, and rollback after a source-sharing violation prevents both copies. The sharing handle is not a running historical process or mapped image. A preflight write-open assertion checks that the old destination denies writes before the sharing-lock transaction.
7. Post-install: the installed native EXE hash must equal `8626e60c908f44848190adc9f3547241c651ed8179601d93e33b91469dc76f20`, and the original probe/version helper must report 1.0.103. The historical baseline hash is `474918134a312ab5b2200e03092c5f34a99b5f2d3847057fe827e3ff193404ca`, reporting 1.0.45.

## Why the whole install function is not called

The original `install_community_release` binds its Windows destination to `std::env::current_exe()`. In a standalone test harness, that is the test runner, not the genuine historical client, and its filename check rejects the runner. Renaming the harness to grok-zh.exe would substitute a different executable and would not establish that the genuine old program performs this update. Therefore this harness does not make that substitution. It declares the isolated destination seam and runs the exact transaction slices instead.

The initial global update policy, target discovery/fetch wiring, runtime current_exe binding, production success receipt and UI block L1232–1239, full outer installer dispatch, cache refresh, and actual user's installation context remain untested by this harness. The genuine old binary's own network updater is also untested by this harness; any separate live baseline must be reported independently with its actual source and destination versions.

## Negative and locking tests

- Wrong but syntactically valid outer SHA: real native stream rejected and partial download removed; old EXE unchanged
- Real stream truncated by one byte: declared-size rejection, cleanup, old EXE unchanged
- Protocol/archive/asset version 1.0.104 with genuine EXE still reporting 1.0.103: reaches actual historical binary-version gate, rejects, removes candidate/archive, old EXE unchanged
- Existing Windows update-lock handle: second acquisition rejected; acquisition succeeds after release
- Genuine old file opened with write-denying/read+delete-sharing handle: original replacement code handles Windows sharing behavior, preserving the old open handle bytes while installed path becomes native
- Genuine candidate opened with exclusive share mode: original replacement copy fails, rename/copy path fails again, rollback restores the original old bytes and old version probe
- Original cancellation module's four tests remain unchanged and run with the suite

No test publishes a release, changes TLS/hosts/trust settings, executes a wrapper or installer, uses a real account, or changes the original input files.

## Isolated Windows execution

The launcher requires:

- GROK_DIRECT_NATIVE_EXE (genuine native EXE with original package companions)
- GROK_DIRECT_NATIVE_ARCHIVE (pinned original native MSVC ZIP for the current-source positive control)
- GROK_DIRECT_HISTORICAL_EXE (genuine historical EXE)
- GROK_DIRECT_HISTORICAL_ARCHIVE (pinned published historical ZIP)

The historical archive/EXE and native fixture identities are verified before execution; the authentic native-ZIP control verifies its pinned outer hash before extraction. `prepare_transaction_fixtures.py` constructs only an explicitly invalid unpublished wrong-version test archive. Generated ZIPs remain ignored and must not be uploaded as products.

The launcher resolves the small standalone crate using historical dependency versions, validates every resolved version/checksum, and builds with `cargo test --locked --no-run --message-format=json`. Then `run_isolated_rust_tests.py` directly launches the returned unit-test executable, not Cargo, under a strict environment allowlist:

- Isolated HOME, USERPROFILE, APPDATA, LOCALAPPDATA, GROK_HOME, TEMP, TMP
- PATH containing only Windows System32 and Windows root
- Basic OS variables, three verified input paths and one result path
- No inherited credential/token variables or developer/Rust/GitHub environment

Thus unchanged `probe_version` inherits an already isolated environment. No global environment is modified by Rust tests, which run serially. Rust builds retain their normal build environment in a separate step. The runtime/profile/temp tree is under ignored runtime-isolation/, outside results/ so residual test binaries cannot enter a results-only artifact upload. The runner reuses the reviewed windows-direct-replacement.py process helper with an 8-minute watchdog, 10-second taskkill timeout, root kill and 5-second wait fallback, and failure if tree/root termination cannot be confirmed; the historical per-probe 30-second deadline remains unchanged.

## Evidence

`results/rust-execution-status.json` requires the native archive test and all nine new functional tests to be successful. It cannot turn the earlier 27-test result into an extension pass. Supporting files include:

- source-provenance-check.json
- transaction-inputs.json
- isolated-test-runner.json (test EXE hash, environment key names, isolated paths, execution result)
- discovery-evidence.json
- historical-baseline-evidence.json
- legacy-transaction-evidence.json
- sharing-lock-evidence.json
- rollback-evidence.json
- cargo-test-build.jsonl and cargo-test-build.log
- rust-tests.log

Even a successful source transaction test does not establish a published native release, end-to-end genuine-old-to-native network upgrade, compatibility of every older updater, or continued self-update behavior of the installed native binary. Those require separate evidence.

The integrated selection-matrix report independently requires all 171 metadata assertions and all four current-source extraction assertions. Its GNU-transport rejection is an expected negative result; a green suite does not claim current direct-GNU transport support.
