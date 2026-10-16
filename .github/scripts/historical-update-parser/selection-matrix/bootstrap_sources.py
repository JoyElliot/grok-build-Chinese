#!/usr/bin/env python3
"""One-time read-only import from a local Git checkout; no remote operations."""
import argparse, hashlib, json, pathlib, subprocess
ROOT=pathlib.Path(__file__).resolve().parent
SOURCE='crates/codegen/xai-grok-update/src/community_release.rs'
PRODUCT='crates/codegen/xai-grok-product/src/lib.rs'
COMMITS={'v1_0_8':'dfcf92b37854f5b2c0dacc66a9e4195f3881dcae','v1_0_45':'3f5d07f8cb2e9be395131296a3efc4635713db7f','current_msvc':'ddede0f98fbbef6349b5040b8390afb9f4d3a5cd'}
def main():
    parser=argparse.ArgumentParser();parser.add_argument('--repo',required=True);args=parser.parse_args()
    manifest={'schema':1,'repository':'JoyElliot/grok-build-Chinese','scope':'Exact unchanged discovery, per-platform asset selection, and current Windows extraction source. Offline synthetic API metadata; pinned real native fixture required for current GNU-transport rejection. No network, activation, installer, or old published executable execution.','revisions':[]}
    for name,commit in COMMITS.items():
        out=ROOT/'sources'/name;out.mkdir(parents=True,exist_ok=True)
        record={'name':name,'commit':commit,'files':[]}
        paths=[(SOURCE,'community_release.rs'),(PRODUCT,'product.rs')]
        if name=='current_msvc':
            paths += [(SOURCE.replace('.rs','/embedded_runtime.rs'),'embedded_runtime.rs'),(SOURCE.replace('.rs','/package_protocol.rs'),'package_protocol.rs')]
        for path,local in paths:
            data=subprocess.check_output(['git','-C',args.repo,'show',f'{commit}:{path}'])
            expected=subprocess.check_output(['git','-C',args.repo,'rev-parse',f'{commit}:{path}'],text=True).strip()
            actual=hashlib.sha1(b'blob '+str(len(data)).encode()+b'\0'+data).hexdigest()
            assert actual==expected
            (out/local).write_bytes(data)
            record['files'].append({'path':str((out/local).relative_to(ROOT)),'source_path':path,'source_url':f'https://github.com/JoyElliot/grok-build-Chinese/blob/{commit}/{path}','git_blob_sha1':actual,'sha256':hashlib.sha256(data).hexdigest(),'bytes':len(data),'compiled':True})
        lines=(out/'community_release.rs').read_text().splitlines(keepends=True)
        def line(prefix):
            positions=[i+1 for i,l in enumerate(lines) if l.startswith(prefix)];assert len(positions)==1,(name,prefix,positions);return positions[0]
        def before(prefix): return line(prefix)-1
        ranges=[[line('const MAX_ASSET_BYTES:'),line('const MAX_ASSET_BYTES:')]]
        if name=='v1_0_8': ranges.append([line('const MAX_SIDECAR_BYTES:'),line('const MAX_SIDECAR_BYTES:')])
        ranges += [[line('const LEGACY_BRIDGE_VERSION:'),line('const LEGACY_BRIDGE_VERSION:')],
                   [line('fn release_repo()'),before('fn releases_api(')],
                   [line('struct ApiRelease {')-1,before('fn current_community_platform()')],
                   [line('fn parse_release_version('),before("fn select_latest_release<'a>(")],
                   [line('fn canonical_release_version('),before('fn release_asset_name(version:')],
                   [line('fn release_includes_macos_assets('),before('fn select_asset(release:')],
                   [line("fn select_latest_compatible_release<'a>("),before('pub(crate) async fn fetch_asset(')]]
        if name=='current_msvc':
            ranges += [[line('const MAX_ARCHIVE_ENTRIES:'),line('const MAX_MANIFEST_BYTES:')],
                       [line('const ONE_CLICK_INSTALLER:'),before('const MACOS_REQUIRED_PACKAGE_FILES:')],
                       [line('const INNER_MANIFEST:'),line('const INNER_MANIFEST:')],
                       [line('fn is_allowed_unicode_package_name('),before('fn release_repo()')],
                       [line('fn normalized_package_name('),before('fn normalized_unix_tar_name(')]]
        record['unchanged_ranges']=ranges
        record['platforms']=['WindowsX86_64Gnu','MacosAarch64','LinuxX86_64Gnu'] if name!='current_msvc' else ['WindowsX86_64Gnu','WindowsX86_64Msvc','WindowsAarch64Msvc','MacosX86_64','MacosAarch64','LinuxX86_64Gnu','LinuxAarch64Gnu']
        manifest['revisions'].append(record)
    (ROOT/'source-provenance.json').write_text(json.dumps(manifest,indent=2)+'\n')
if __name__=='__main__':main()
