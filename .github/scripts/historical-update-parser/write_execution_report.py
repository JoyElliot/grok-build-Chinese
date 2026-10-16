#!/usr/bin/env python3
import argparse,hashlib,json,pathlib,re
base=pathlib.Path(__file__).resolve().parent
p=argparse.ArgumentParser();p.add_argument('--exit-code',type=int,required=True);p.add_argument('--stage',required=True);args=p.parse_args()
logpath=base/'results/rust-tests.log'; raw=logpath.read_bytes() if logpath.exists() else b''
# PowerShell 7 writes UTF-8. Handle Windows PowerShell UTF-16 explicitly too.
text=raw.decode('utf-16' if raw.startswith((b'\xff\xfe',b'\xfe\xff')) else 'utf-8-sig',errors='replace')
summaries=re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored;',text)
native_ok=bool(re.search(r'test historical::compatibility_tests::native_msvc_payload_full_package_extracts_byte_identically\s+\.\.\. ok',text))
fixturepath=base/'results/native-fixture-provenance.json';fixture=json.loads(fixturepath.read_text(encoding='utf-8')) if fixturepath.exists() else {}
passed=args.exit_code==0 and native_ok and bool(summaries) and all(x[0]=='ok' and x[2]=='0' for x in summaries)
report={'status':'PASSED' if passed else 'BLOCKED_OR_FAILED','stage':args.stage,'cargo_exit_code':args.exit_code,'parser_commit':'3f5d07f8cb2e9be395131296a3efc4635713db7f','test_summaries':[{'status':s,'passed':int(n),'failed':int(f),'ignored':int(i)} for s,n,f,i in summaries],'native_full_package_test_passed':native_ok,'native_executable_sha256':fixture.get('native_executable_sha256'),'actual_runtime_target':fixture.get('native_target'),'compatibility_transport_selector':fixture.get('protocol_platform'),'fixture_archive_sha256':fixture.get('fixture_archive_sha256'),'rust_log_sha256':hashlib.sha256(raw).hexdigest(),'intended_coverage_if_tests_pass':['Exact historical Rust parser behavior for local fixtures','Inner archive hashes and byte-identical EXE extraction'],'does_not_prove':['Live immutable GitHub release or download validation','Executable activation/replacement on Windows','Installer execution','Current replacement binary updater routing','v1.0.8 six-asset bridge availability']}
(base/'results/rust-execution-status.json').write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps(report,indent=2))
raise SystemExit(0 if passed else 1)
