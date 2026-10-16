#!/usr/bin/env python3
"""Pin genuine input files; prepare one explicitly invalid version fixture. No EXE executes here."""
import hashlib,json,os,pathlib,shutil,zipfile
BASE=pathlib.Path(__file__).resolve().parent
OLD_HASH='474918134a312ab5b2200e03092c5f34a99b5f2d3847057fe827e3ff193404ca'
OLD_ZIP_HASH='f074aea7d5de99d469e4beb821407926a46076225f2bf986d789ce1b118159d8'
def sha(p):
    with p.open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def main():
    old=pathlib.Path(os.environ['GROK_DIRECT_HISTORICAL_EXE']).resolve();archive=pathlib.Path(os.environ['GROK_DIRECT_HISTORICAL_ARCHIVE']).resolve()
    assert sha(old)==OLD_HASH and sha(archive)==OLD_ZIP_HASH,'Historical artifact identity mismatch'
    snapshot=json.loads((BASE/'metadata/historical-release-v1.0.45.json').read_text(encoding='utf-8'))
    asset=next(x for x in snapshot['assets'] if x['name']=='grok-zh-1.0.45-windows-x86_64-gnu.zip')
    assert snapshot['immutable'] and asset['digest']=='sha256:'+OLD_ZIP_HASH and asset['size']==archive.stat().st_size
    native=BASE/'fixtures/grok-zh-1.0.103-windows-x86_64-gnu.zip';target=BASE/'fixtures/grok-zh-1.0.104-windows-x86_64-gnu.zip'
    oldroot=native.name[:-4]+'/';newroot=target.name[:-4]+'/'
    with zipfile.ZipFile(native) as src:
        original=src.read(oldroot+'BUILD-INFO.txt').decode('utf-8')
        human,rest=original.split('GROK-UPDATE-PROTOCOL-BEGIN\n');body,tail=rest.split('\nGROK-UPDATE-PROTOCOL-END',1);protocol=json.loads(body)
        protocol['version']='1.0.104'
        human+='NEGATIVE TEST FIXTURE: claims release version 1.0.104 while unchanged genuine EXE reports 1.0.103; must be rejected by binary version probe.\n'
        info=(human+'GROK-UPDATE-PROTOCOL-BEGIN\n'+json.dumps(protocol,indent=2)+'\nGROK-UPDATE-PROTOCOL-END'+tail).encode()
        manifest={line.split('  ',1)[1]:line.split('  ',1)[0] for line in src.read(oldroot+'SHA256SUMS.txt').decode().splitlines()}
        manifest['BUILD-INFO.txt']=hashlib.sha256(info).hexdigest()
        with zipfile.ZipFile(target,'w',compression=zipfile.ZIP_STORED) as dst:
            for entry in src.infolist():
                relative=entry.filename[len(oldroot):]
                if relative in ['BUILD-INFO.txt','SHA256SUMS.txt']:continue
                with src.open(entry) as incoming,dst.open(newroot+relative,'w') as outgoing:shutil.copyfileobj(incoming,outgoing,1024*1024)
            dst.writestr(newroot+'BUILD-INFO.txt',info)
            dst.writestr(newroot+'SHA256SUMS.txt',''.join(f'{digest}  {name}\n' for name,digest in sorted(manifest.items())))
    release={'tag_name':'release-v1.0.104','body':'NEGATIVE LOCAL FIXTURE ONLY: genuine EXE version intentionally wrong; unpublished','draft':False,'prerelease':False,'immutable':True,'assets':[{'name':target.name,'browser_download_url':'https://github.com/JoyElliot/grok-build-Chinese/releases/download/release-v1.0.104/'+target.name,'size':target.stat().st_size,'digest':'sha256:'+sha(target),'state':'uploaded'}]}
    (BASE/'fixtures/wrong-version-release.json').write_text(json.dumps(release,indent=2)+'\n',encoding='utf-8')
    report={'historical_exe_sha256':OLD_HASH,'historical_archive_sha256':OLD_ZIP_HASH,'historical_metadata':'Actual GitHub snapshot stored at metadata/historical-release-v1.0.45.json','negative_native_fixture':{'claimed_version':'1.0.104','actual_exe_version':'1.0.103','native_bytes_unchanged':True,'archive_sha256':sha(target)},'execution':'PENDING WINDOWS TESTS'}
    (BASE/'results/transaction-inputs.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
    print(json.dumps(report,indent=2))
if __name__=='__main__':main()
