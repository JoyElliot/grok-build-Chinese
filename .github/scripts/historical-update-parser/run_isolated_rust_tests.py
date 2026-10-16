#!/usr/bin/env python3
"""Run built Rust tests using the shared hardened process/cleanup implementation."""
import hashlib,importlib.util,json,os,pathlib,re,sys,time
BASE=pathlib.Path(__file__).resolve().parent
RESULTS=BASE/'results'

def load_direct_helper():
    path=BASE.parent/'windows-direct-replacement.py'
    spec=importlib.util.spec_from_file_location('historical_direct_process_helper',path)
    assert spec and spec.loader,'Shared bounded process helper is unavailable'
    module=importlib.util.module_from_spec(spec);sys.modules[spec.name]=module;spec.loader.exec_module(module)
    return module,path

def main():
    RESULTS.mkdir(exist_ok=True)
    report={'status':'STARTING','exit_code':1,'timeout_seconds':480,'credential_environment_inherited':False,'cargo_or_developer_path_inherited':False,'source_probe_environment_mutated':False,'parallel_environment_mutation':False}
    started=time.monotonic();code=1;failure=None
    try:
        if os.name!='nt':raise RuntimeError('Native Windows is required; no substitute execution is allowed.')
        direct,helper_path=load_direct_helper()
        report['shared_process_helper']=str(helper_path)
        report['shared_process_helper_sha256']=hashlib.sha256(helper_path.read_bytes()).hexdigest()
        records=[]
        for line in (RESULTS/'cargo-test-build.jsonl').read_text(encoding='utf-8-sig').splitlines():
            try:record=json.loads(line)
            except json.JSONDecodeError:continue
            if record.get('reason')=='compiler-artifact' and record.get('profile',{}).get('test') and record.get('executable') and record.get('target',{}).get('name')=='historical_community_parser_compatibility':records.append(record)
        executables=sorted({str(pathlib.Path(x['executable']).resolve()) for x in records})
        assert len(executables)==1,f'Expected one parser unit-test EXE, got {executables}'
        root=BASE/'runtime-isolation';root.mkdir(parents=True,exist_ok=True)
        env=direct.isolated_environment(root)
        for name in ['GROK_DIRECT_HISTORICAL_EXE','GROK_DIRECT_HISTORICAL_ARCHIVE','GROK_DIRECT_NATIVE_ARCHIVE']:
            env[name]=str(pathlib.Path(os.environ[name]).resolve())
        env['GROK_HISTORICAL_TEST_RESULTS']=str(RESULTS)
        exe=pathlib.Path(executables[0]);command=[str(exe),'--nocapture','--test-threads=1']
        with exe.open('rb') as file:exe_hash=hashlib.file_digest(file,'sha256').hexdigest()
        report.update({'command':command,'test_exe_sha256':exe_hash,'environment_keys':sorted(env),'path':env['PATH'],'profiles':{k:env[k] for k in ['HOME','USERPROFILE','APPDATA','LOCALAPPDATA','GROK_HOME','TEMP','TMP']}})
        result=direct.run_command(command,root,env,'historical-rust-tests',RESULTS,timeout_seconds=480)
        report['process_result']=result;code=result['returncode'];report['status']='PASSED' if code==0 else 'FAILED'
    except Exception as error:
        failure=error;report['error_type']=type(error).__name__;report['error']=str(error);report['status']='FAILED'
        match=re.search(r'failed, exit=(-?\d+)',str(error))
        if match:code=int(match.group(1))
        elif type(error).__name__=='UnconfirmedProcessTermination':code=125
        elif 'exceeded 480 seconds' in str(error):code=124
        else:code=1
    finally:
        # Keep both original helper logs and the combined Rust assertion log.
        # This runs even when the helper reports timeout, cleanup failure or nonzero exit.
        try:
            stdout=RESULTS/'historical-rust-tests.stdout.txt';stderr=RESULTS/'historical-rust-tests.stderr.txt'
            combined=(stdout.read_bytes() if stdout.exists() else b'')+(stderr.read_bytes() if stderr.exists() else b'')
            if failure is not None:combined+=f'\nRunner failure: {type(failure).__name__}: {failure}\n'.encode('utf-8',errors='replace')
            (RESULTS/'rust-tests.log').write_bytes(combined)
        except OSError as error:
            report['combined_log_error']=str(error);code=code or 1
        cleanup=RESULTS/'historical-rust-tests.cleanup.txt'
        if cleanup.exists():
            try:report['bounded_cleanup']=json.loads(cleanup.read_text(encoding='utf-8'))
            except (OSError,ValueError) as error:report['cleanup_report_read_error']=str(error);code=code or 1
        report['exit_code']=code;report['elapsed_seconds']=round(time.monotonic()-started,3)
        if code!=0:report['status']='FAILED'
        try:(RESULTS/'isolated-test-runner.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
        except OSError as error:
            print(f'Runner report write failed: {error}',file=sys.stderr);code=code or 1
    print(json.dumps(report,indent=2))
    raise SystemExit(code)
if __name__=='__main__':main()
