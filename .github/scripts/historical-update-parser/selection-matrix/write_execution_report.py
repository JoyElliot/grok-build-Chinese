#!/usr/bin/env python3
"""A green assertion suite does not imply a supported direct-transport next update."""
import argparse, hashlib, json, pathlib, re
ROOT=pathlib.Path(__file__).resolve().parent
SOURCE_BASE='https://github.com/JoyElliot/grok-build-Chinese/blob/ddede0f98fbbef6349b5040b8390afb9f4d3a5cd/crates/codegen/xai-grok-update/src/'

def main():
    p=argparse.ArgumentParser();p.add_argument('--exit-code',type=int);p.add_argument('--stage',default='static preflight only');p.add_argument('--log',default='results/rust-tests.log');args=p.parse_args()
    cases=json.loads((ROOT/'fixtures/selection-matrix.json').read_text())['cases']
    path=ROOT/args.log;raw=path.read_bytes() if path.exists() else b'';text=raw.decode('utf-8',errors='replace')
    required={c['id'] for c in cases}
    passed=set(re.findall(r'^test (?:[A-Za-z0-9_]+::)*matrix_tests::([A-Za-z0-9_]+) \.\.\. ok\s*$',text,re.M))
    extraction_ids={
      'authentic_native_msvc_archive_extracts_identical_native_exe',
      'direct_gnu_transport_without_embedded_runtime_is_rejected',
      'native_named_package_extracts_declared_inert_bytes',
      'real_native_gnu_transport_is_rejected_for_current_msvc',
    }
    extraction_passed=set(re.findall(r'^test (?:[A-Za-z0-9_]+::)*current_msvc::extraction_tests::([A-Za-z0-9_]+) \.\.\. ok\s*$',text,re.M))
    summaries=re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored',text)
    good=args.exit_code==0 and required<=passed and extraction_ids<=extraction_passed and any(s=='ok' and int(n)>=len(required)+len(extraction_ids) and int(f)==0 and int(i)==0 for s,n,f,i in summaries)
    assertion_status='PASSED' if good else ('NOT_EXECUTED' if args.exit_code is None else 'BLOCKED_OR_FAILED')
    report={
      'schema':1,'stage':args.stage,'test_suite_status':assertion_status,
      'cargo_exit_code':args.exit_code,'required_matrix_assertions':len(required),
      'observed_successful_matrix_assertions':len(required & passed),
      'required_extraction_assertions':len(extraction_ids),
      'observed_successful_extraction_assertions':sorted(extraction_ids & extraction_passed),
      'missing_successful_extraction_assertions':sorted(extraction_ids-extraction_passed),
      'missing_successful_assertions':sorted(required-passed),
      'fixture_kind':'SYNTHETIC_OFFLINE_API_METADATA',
      'rust_log_sha256':hashlib.sha256(raw).hexdigest() if raw else None,
      'product_continuity':{
        'current_native_direct_transport_supported':False,
        'scope':'Pinned ddede0f updater, requesting Windows x64 MSVC, GNU-named package with direct native EXE and actual_runtime_target but no embedded_runtime descriptor',
        'status':'UNSUPPORTED_BY_PINNED_SOURCE',
        'basis':'Exact current-source test with pinned real native GNU-transport fixture' if 'real_native_gnu_transport_is_rejected_for_current_msvc' in extraction_passed and args.exit_code==0 else 'Exact source inspection only until required extraction assertions execute. Selection accepts potential GNU fallback, but extraction passes require_embedded=true and rejects absent embedded_runtime. actual_runtime_target is not a routing field of PackageProtocol.',
        'extraction_execution_in_this_harness':assertion_status,
        'native_named_parser_extraction_control':'PASSED_INERT_BYTES_ONLY' if 'native_named_package_extracts_declared_inert_bytes' in extraction_passed and args.exit_code==0 else 'NOT_VERIFIED',
        'authentic_native_named_archive_extraction':'PASSED' if 'authentic_native_msvc_archive_extracts_identical_native_exe' in extraction_passed and args.exit_code==0 else 'NOT_VERIFIED',
        'pinned_original_native_archive_sha256':'802fbd20fee858fc90521438c23d5d9934370c5a97815680c208305e19a2b41d',
        'pinned_real_native_exe_sha256':'8626e60c908f44848190adc9f3547241c651ed8179601d93e33b91469dc76f20',
        'source_evidence':[
          SOURCE_BASE+'community_release.rs#L1090-L1097',
          SOURCE_BASE+'community_release/embedded_runtime.rs#L24-L31',
          SOURCE_BASE+'community_release.rs#L1131-L1134',
          SOURCE_BASE+'community_release/package_protocol.rs#L10-L21'
        ],
        'current_native_named_asset_selection':assertion_status,
        'current_gnu_named_potential_fallback_selection':assertion_status,
        'v108_frozen_six_asset_bridge_selection':assertion_status,
        'v1045_selected_platform_successor_selection':assertion_status,
        'live_bridge_inventory_and_availability':'NOT_CHECKED',
        'native_executable_followup_network_update':'NOT_EXECUTED',
        'published_old_binary_network_update':'NOT_EXECUTED',
      },
      'interpretation':'Expected rejections are success conditions for the tests. Even if all tests pass, direct native EXE in GNU transport remains unsupported for the current MSVC next-update extraction route. Native-named metadata selection and frozen-bridge metadata coverage are separate findings.',
      'does_not_prove':['A published release exists or remains immutable','Downloaded asset hash matches real bytes','A historical executable actually queried GitHub','The extracted native executable runs or completes a distinct subsequent-version update','Executable activation or replacement succeeds','A full-product build or runtime passes','The live frozen bridge inventory matches these synthetic fixtures']
    }
    (ROOT/'results').mkdir(exist_ok=True);(ROOT/'results/selection-execution-report.json').write_text(json.dumps(report,indent=2)+'\n')
    print(f"{assertion_status}: {len(required & passed)}/{len(required)} exact-source selection assertions and {len(extraction_ids & extraction_passed)}/{len(extraction_ids)} extraction assertions; current_native_direct_transport_supported=false")
    if args.exit_code is not None and not good:raise SystemExit(1)
if __name__=='__main__':main()
