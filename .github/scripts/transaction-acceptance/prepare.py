#!/usr/bin/env python3
"""CI-only pinned inputs and portable test bundle; never executes a product."""
import argparse, hashlib, json, pathlib, shutil, subprocess, struct, urllib.request, urllib.parse, zipfile
ROOT=pathlib.Path(__file__).resolve().parents[3]
REPO='JoyElliot/grok-build-Chinese'
RUN=40156800532
ARTIFACT=12584849702
SOURCE='0eb78dd950ad376ea8a1f1df3d2e8719f14c42e7'
PINS={
 'product-artifact.zip':'102a9a1e6871c8f77701d37deb596d5bacc7cc78f1cd7fbc83490b598fcc18fc',
 'historical-gnu.zip':'f074aea7d5de99d469e4beb821407926a46076225f2bf986d789ce1b118159d8',
 'native-msvc.zip':'de9e0ea609a8494ccde3024b0b9b6a669f12387d5a92b39e0d73e34d5ab770ff',
 'direct-msvc.zip':'ff7ee4339f578b290d2f7ec42236f75889701ae0badc92d462910cb962c10433',
}
def sha(path):
 with pathlib.Path(path).open('rb') as f:return hashlib.file_digest(f,'sha256').hexdigest()
def checked(path,name):
 if sha(path)!=PINS[name]:raise ValueError(f'Pinned SHA256 mismatch: {name}')
def gh(path):
 return json.loads(subprocess.check_output(['gh','api',f'repos/{REPO}/{path}'],timeout=120))
class GitHubRedirect(urllib.request.HTTPRedirectHandler):
 def redirect_request(self,req,fp,code,msg,headers,newurl):
  verify_url(newurl)
  return super().redirect_request(req,fp,code,msg,headers,newurl)
def verify_url(url):
 p=urllib.parse.urlsplit(url)
 if p.scheme!='https' or p.port not in (None,443) or p.hostname not in ('github.com','release-assets.githubusercontent.com','objects.githubusercontent.com'):
  raise ValueError('Untrusted public release URL')
def download(directory):
 directory.mkdir(parents=True,exist_ok=False)
 run=gh(f'actions/runs/{RUN}')
 if run['head_sha']!=SOURCE or run['conclusion']!='success' or run['repository']['full_name']!=REPO:raise ValueError('Product run provenance mismatch')
 artifact=gh(f'actions/artifacts/{ARTIFACT}')
 if artifact['expired'] or artifact['workflow_run']['id']!=RUN or artifact['digest']!='sha256:'+PINS['product-artifact.zip']:raise ValueError('Product artifact provenance mismatch')
 with (directory/'product-artifact.zip').open('wb') as output:
  subprocess.run(['gh','api',f'repos/{REPO}/actions/artifacts/{ARTIFACT}/zip'],stdout=output,check=True,timeout=300)
 checked(directory/'product-artifact.zip','product-artifact.zip')
 release=gh('releases/407819157')
 tag=gh('git/ref/tags/release-v1.0.45')['object']
 if tag['type']=='tag':tag=gh('git/tags/'+tag['sha'])['object']
 if tag['type']!='commit' or tag['sha']!='3f5d07f8cb2e9be395131296a3efc4635713db7f':raise ValueError('Historical tag/source mismatch')
 old=next(a for a in release['assets'] if a['id']==624680447)
 url=f'https://github.com/{REPO}/releases/download/release-v1.0.45/grok-zh-1.0.45-windows-x86_64-gnu.zip'
 if release['tag_name']!='release-v1.0.45' or not release['immutable'] or old['browser_download_url']!=url or old['digest']!='sha256:'+PINS['historical-gnu.zip'] or old['size']!=65719142:raise ValueError('Historical immutable release provenance mismatch')
 verify_url(url)
 with urllib.request.build_opener(GitHubRedirect()).open(url,timeout=60) as incoming,(directory/'historical-gnu.zip').open('wb') as output:
  size=0
  while block:=incoming.read(1024*1024):
   size+=len(block)
   if size>65719142:raise ValueError('Historical release exceeded pinned size')
   output.write(block)
 checked(directory/'historical-gnu.zip','historical-gnu.zip')
 with zipfile.ZipFile(directory/'product-artifact.zip') as z:
  for name,member in [('native-msvc.zip','dist/grok-zh-1.0.104-windows-x86_64-msvc.zip'),('direct-msvc.zip','direct-dist/grok-zh-1.0.104-windows-x86_64-gnu.zip')]:
   with z.open(member) as incoming,(directory/name).open('wb') as output:shutil.copyfileobj(incoming,output)
   checked(directory/name,name)
 evidence={'artifact_source_commit':SOURCE,'artifact_run':RUN,'artifact_id':ARTIFACT,'artifact_digest':PINS['product-artifact.zip'],'historical_source_commit':'3f5d07f8cb2e9be395131296a3efc4635713db7f','historical_release_id':407819157,'historical_asset_id':624680447,'historical_release_immutable':True,'archive_hashes':PINS,'network_scope':'CI downloads original GitHub files only; user runner is offline','limits':['Simulated API metadata and local stream transport','Not unmodified historical EXE online migration','Current MSVC same-version reinstall is not a distinct later-version upgrade']}
 (directory/'input-provenance.json').write_text(json.dumps(evidence,indent=2)+'\n',encoding='utf-8')
def check_portable_pe(path):
 data=path.read_bytes()
 if data[:2]!=b'MZ':raise ValueError('Test binary is not PE')
 pe=struct.unpack_from('<I',data,0x3c)[0]
 if data[pe:pe+4]!=b'PE\0\0':raise ValueError('Invalid PE signature')
 machine,count=struct.unpack_from('<HH',data,pe+4)
 size=struct.unpack_from('<H',data,pe+20)[0];opt=pe+24
 if machine!=0x8664 or struct.unpack_from('<H',data,opt)[0]!=0x20b:raise ValueError('Test binary must be native x64 PE32+')
 sections=[]
 for i in range(count):
  section=opt+size+i*40
  virtual_size,rva,raw_size,raw=struct.unpack_from('<IIII',data,section+8)
  sections.append((rva,max(virtual_size,raw_size),raw,raw_size))
 def offset(rva):
  for start,length,raw,raw_size in sections:
   if start<=rva<start+length and rva-start<raw_size:return raw+rva-start
  raise ValueError('PE import RVA outside file-backed sections')
 imports_rva,imports_size=struct.unpack_from('<II',data,opt+112+8)
 delay_rva,delay_size=struct.unpack_from('<II',data,opt+112+13*8)
 if delay_rva or delay_size:raise ValueError('Unexpected delay imports in standalone harness')
 names=[]
 if imports_rva:
  base=offset(imports_rva)
  for i in range(min(imports_size//20+1,1024)):
   record=struct.unpack_from('<IIIII',data,base+i*20)
   if not any(record):break
   name_offset=offset(record[3]);end=data.index(b'\0',name_offset,name_offset+256)
   names.append(data[name_offset:end].decode('ascii').lower())
  else:raise ValueError('Unterminated PE import table')
 allowed={'kernel32.dll','ntdll.dll','advapi32.dll','bcrypt.dll','bcryptprimitives.dll','crypt32.dll','ws2_32.dll','userenv.dll','secur32.dll','user32.dll','shell32.dll','ole32.dll','oleaut32.dll','iphlpapi.dll','ncrypt.dll','dbghelp.dll','psapi.dll','shlwapi.dll','version.dll','gdi32.dll','rpcrt4.dll','winmm.dll','normaliz.dll','combase.dll','powrprof.dll'}
 if not names or any(n not in allowed and not n.startswith('api-ms-win-') for n in names):raise ValueError(f'Non-system runtime dependency: {names}')
 return sorted(names)
def bundle(inputs,build_log,out):
 out.mkdir(parents=True,exist_ok=False);(out/'inputs').mkdir();(out/'bin').mkdir()
 for name in ['historical-gnu.zip','native-msvc.zip','direct-msvc.zip']:
  checked(inputs/name,name);shutil.copyfile(inputs/name,out/'inputs'/name)
 shutil.copyfile(inputs/'input-provenance.json',out/'input-provenance.json')
 harness=ROOT/'.github/scripts/direct-transaction-validation'
 shutil.copyfile(harness/'results/source-provenance.json',out/'source-provenance.json')
 shutil.copyfile(harness/'README.md',out/'TRANSACTION-SOURCE-COVERAGE.md')
 expected={'legacy-transaction','current-transaction'};executables={}
 for line in build_log.read_text(encoding='utf-8-sig').splitlines():
  try:r=json.loads(line)
  except ValueError:continue
  name=r.get('target',{}).get('name')
  if r.get('reason')=='compiler-artifact' and name in expected and r.get('profile',{}).get('test') and r.get('executable'):executables[name]=pathlib.Path(r['executable'])
 if set(executables)!=expected:raise ValueError(f'Missing exact test executables: {executables}')
 imports={}
 for name,source in executables.items():
  imports[name]=check_portable_pe(source);shutil.copyfile(source,out/'bin'/(name+'.exe'))
 for name in ['Run-MigrationTransaction.ps1','Windows迁移验收说明.txt']:shutil.copyfile(pathlib.Path(__file__).parent/name,out/name)
 provenance=json.loads((out/'input-provenance.json').read_text());provenance['test_binary_imports']=imports;provenance['harness_source_commit']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
 (out/'input-provenance.json').write_text(json.dumps(provenance,indent=2)+'\n',encoding='utf-8')
 manifest={p.relative_to(out).as_posix():sha(p) for p in sorted(out.rglob('*')) if p.is_file()}
 (out/'SHA256SUMS.json').write_text(json.dumps(manifest,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
 print(json.dumps({'bundle':str(out),'files':len(manifest),'targets':sorted(executables)},indent=2))
if __name__=='__main__':
 p=argparse.ArgumentParser();sub=p.add_subparsers(dest='command',required=True)
 d=sub.add_parser('download');d.add_argument('directory',type=pathlib.Path)
 b=sub.add_parser('bundle');b.add_argument('inputs',type=pathlib.Path);b.add_argument('build_log',type=pathlib.Path);b.add_argument('output',type=pathlib.Path)
 a=p.parse_args()
 if a.command=='download':download(a.directory)
 else:bundle(a.inputs,a.build_log,a.output)
