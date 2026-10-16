#!/usr/bin/env python3
"""Create isolated test data only; never replace/install/run an EXE or publish an asset."""
import argparse, hashlib, json, pathlib, re, zipfile
BASE=pathlib.Path(__file__).resolve().parent
EXPECTED_EXE='8626e60c908f44848190adc9f3547241c651ed8179601d93e33b91469dc76f20'
def digest(p):
    h=hashlib.sha256()
    with p.open('rb') as f:
        for b in iter(lambda:f.read(1024*1024),b''):h.update(b)
    return h.hexdigest()
def main():
    p=argparse.ArgumentParser();p.add_argument('native_package',type=pathlib.Path);args=p.parse_args()
    native=args.native_package.resolve(); exe=native/'grok-zh.exe'
    assert digest(exe)==EXPECTED_EXE, 'Unexpected executable bytes; this harness pins the verified CI artifact'
    original=(native/'BUILD-INFO.txt').read_text(encoding='utf-8-sig')
    assert 'Target: x86_64-pc-windows-msvc' in original
    assert 'Commit: ddede0f98fbbef6349b5040b8390afb9f4d3a5cd' in original
    protocol={'schema':1,'version':'1.0.103','platform':'x86_64-pc-windows-gnu','mode':'executable-only','manifest':'SHA256SUMS.txt','executable':'grok-zh.exe','installer':'Install-GrokZh.ps1','actual_runtime_target':'x86_64-pc-windows-msvc','compatibility_transport':'GNU-named transport for legacy GNU updater; executable payload is native MSVC'}
    human=original.split('GROK-UPDATE-PROTOCOL-BEGIN')[0]
    human+='Fixture: isolated historical-parser verification; not a published release\nCompatibility transport selector: x86_64-pc-windows-gnu\nActual executable runtime target: x86_64-pc-windows-msvc\n\n'
    info=(human+'GROK-UPDATE-PROTOCOL-BEGIN\n'+json.dumps(protocol,indent=2)+'\nGROK-UPDATE-PROTOCOL-END\n').encode()
    name='grok-zh-1.0.103-windows-x86_64-gnu.zip'; root=name[:-4]
    files=[x for x in sorted(native.rglob('*')) if x.is_file() and x.relative_to(native).as_posix() not in ('BUILD-INFO.txt','SHA256SUMS.txt')]
    hashes={x.relative_to(native).as_posix():digest(x) for x in files}; hashes['BUILD-INFO.txt']=hashlib.sha256(info).hexdigest()
    manifest=''.join(f'{h}  {n}\n' for n,h in sorted(hashes.items())).encode()
    target=BASE/'fixtures'/name
    with zipfile.ZipFile(target,'w',compression=zipfile.ZIP_STORED) as z:
        for x in files:z.write(x,root+'/'+x.relative_to(native).as_posix())
        z.writestr(root+'/BUILD-INFO.txt',info); z.writestr(root+'/SHA256SUMS.txt',manifest)
    outer=digest(target)
    # Synthetic API metadata exercises the parser. No such release is asserted to exist.
    release={'tag_name':'release-v1.0.103','body':'Synthetic local API fixture; not fetched or published','draft':False,'prerelease':False,'immutable':True,'assets':[{'name':name,'browser_download_url':'https://github.com/JoyElliot/grok-build-Chinese/releases/download/release-v1.0.103/'+name,'size':target.stat().st_size,'digest':'sha256:'+outer,'state':'uploaded'}]}
    (BASE/'fixtures/native-release.json').write_text(json.dumps(release,indent=2)+'\n')
    (BASE/'fixtures/native-build-info.txt').write_bytes(info)
    (BASE/'fixtures/native-sha256sums.txt').write_bytes(manifest)
    result={'status':'FIXTURE PREPARED; RUST PARSER NOT YET EXECUTED','native_executable_sha256':EXPECTED_EXE,'native_target':'x86_64-pc-windows-msvc','native_source_commit':'ddede0f98fbbef6349b5040b8390afb9f4d3a5cd','native_workflow_run':38842814822,'native_workflow_attempt':1,'native_artifact':12022879975,'native_artifact_sha256':'4a3d80bac40ba3c63bfeb00f4b97976f6727842463a67c10d7d4b15749278f33','native_inner_zip_sha256':'802fbd20fee858fc90521438c23d5d9934370c5a97815680c208305e19a2b41d','fixture_archive':str(target),'fixture_archive_bytes':target.stat().st_size,'fixture_archive_sha256':outer,'protocol_platform':'x86_64-pc-windows-gnu','protocol_actual_runtime_target':'x86_64-pc-windows-msvc','api_metadata':'synthetic local fixture, not authenticated live GitHub metadata','changed_package_files':['BUILD-INFO.txt','SHA256SUMS.txt'],'unchanged_payload_hashes':hashes}
    (BASE/'results/native-fixture-provenance.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps({k:v for k,v in result.items() if k not in ['unchanged_payload_hashes']},indent=2))
if __name__=='__main__':main()
