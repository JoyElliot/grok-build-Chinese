#!/usr/bin/env python3
import argparse,hashlib,json,pathlib,re
base=pathlib.Path(__file__).resolve().parent
p=argparse.ArgumentParser();p.add_argument('--exit-code',type=int,required=True);p.add_argument('--stage',required=True);args=p.parse_args()
logpath=base/'results/rust-tests.log';raw=logpath.read_bytes() if logpath.exists() else b''
text=raw.decode('utf-16' if raw.startswith((b'\xff\xfe',b'\xfe\xff')) else 'utf-8-sig',errors='replace')
summaries=re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;',text)
required={
 'msvc_host_routing_limit':'historical::compatibility_tests::historical_current_platform_rejects_native_msvc_build',
 'native_archive_extraction':'historical::compatibility_tests::native_msvc_payload_full_package_extracts_byte_identically',
 'real_metadata_and_candidate_selection':'historical::transaction::discovery_uses_real_historical_metadata_and_explicit_unpublished_candidate',
 'genuine_old_stream_extract_probe':'historical::transaction::genuine_old_archive_passes_unchanged_stream_hash_extraction_and_probe',
 'source_transaction_and_native_probe':'historical::transaction::full_isolated_legacy_transaction_upgrades_genuine_old_to_native_and_probes',
 'wrong_outer_hash_cleanup':'historical::transaction::wrong_outer_hash_rejects_real_stream_and_cleans_without_changing_old_exe',
 'truncated_stream_cleanup':'historical::transaction::truncated_real_stream_rejects_and_cleans_without_changing_old_exe',
 'wrong_binary_version_cleanup':'historical::transaction::wrong_real_native_version_rejects_at_historical_probe_gate_and_cleans',
 'update_lock_contention':'historical::transaction::original_windows_update_lock_rejects_contention_and_releases',
 'old_file_sharing_lock':'historical::transaction::genuine_old_file_sharing_lock_exercises_original_rename_fallback',
 'original_copy_failure_rollback':'historical::transaction::original_replacement_rolls_back_if_source_copy_stays_sharing_locked',
}
checks={name:bool(re.search(r'(?m)^test '+re.escape(test)+r'\s+\.\.\. ok\s*$',text)) for name,test in required.items()}
fixturepath=base/'results/native-fixture-provenance.json';fixture=json.loads(fixturepath.read_text(encoding='utf-8')) if fixturepath.exists() else {}
runnerpath=base/'results/isolated-test-runner.json';runner=json.loads(runnerpath.read_text(encoding='utf-8')) if runnerpath.exists() else {}
passed=args.exit_code==0 and all(checks.values()) and bool(summaries) and all(x[0]=='ok' and x[2]=='0' for x in summaries) and runner.get('exit_code')==0 and runner.get('credential_environment_inherited') is False
report={
 'status':'PASSED' if passed else 'BLOCKED_OR_FAILED','stage':args.stage,'test_process_exit_code':args.exit_code,
 'build_command':'cargo test --locked --no-run --message-format=json; then direct isolated test EXE',
 'parser_commit':'3f5d07f8cb2e9be395131296a3efc4635713db7f',
 'test_summaries':[{'status':s,'passed':int(n),'failed':int(f),'ignored':int(i)} for s,n,f,i in summaries],
 'required_functional_checks':checks,'required_test_names':required,
 'native_full_package_test_passed':checks['native_archive_extraction'],'native_executable_sha256':fixture.get('native_executable_sha256'),
 'historical_executable_sha256':'474918134a312ab5b2200e03092c5f34a99b5f2d3847057fe827e3ff193404ca',
 'actual_runtime_target':fixture.get('native_target'),'compatibility_transport_selector':fixture.get('protocol_platform'),'fixture_archive_sha256':fixture.get('fixture_archive_sha256'),
 'rust_log_sha256':hashlib.sha256(raw).hexdigest(),
 'intended_coverage_if_tests_pass':['Exact historical metadata compatibility selection','Real archive byte streams through unchanged outer size/hash verifier','Inner archive verification and byte-identical extraction','Exact 30-second probe helpers and version gate','Unchanged Windows transaction slices with isolated destination/transport seams','Original update-lock contention, sharing-file replacement and rollback','Native post-install probe'],
 'not_executed_by_this_harness':['Whole install_community_release function including current_exe binding and success receipt','Genuine historical binary invoking its own network update command','Live HTTPS API pagination/download/redirect/trust adapter','Running historical process locking its mapped EXE image','Current replacement binary updater routing','Installer execution','Production release publication or availability','v1.0.8 bridge availability'],
 'evidence_files':['source-provenance-check.json','isolated-test-runner.json','transaction-inputs.json','discovery-evidence.json','historical-baseline-evidence.json','legacy-transaction-evidence.json','sharing-lock-evidence.json','rollback-evidence.json','rust-tests.log']}
(base/'results/rust-execution-status.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps(report,indent=2));raise SystemExit(0 if passed else 1)
