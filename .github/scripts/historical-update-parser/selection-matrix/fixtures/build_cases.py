#!/usr/bin/env python3
"""Synthetic metadata and explicit expectations, not an implementation of selection."""
import copy, json, pathlib
ROOT=pathlib.Path(__file__).resolve().parent.parent
REPO='JoyElliot/grok-build-Chinese'
GNU='WindowsX86_64Gnu'; MSVC='WindowsX86_64Msvc'; ARM='WindowsAarch64Msvc'; MAC='MacosAarch64'; INTEL='MacosX86_64'; LINUX='LinuxX86_64Gnu'; LINUX_ARM='LinuxAarch64Gnu'
SUFFIX={GNU:'windows-x86_64-gnu.zip',MSVC:'windows-x86_64-msvc.zip',ARM:'windows-aarch64-msvc.zip',MAC:'macos-aarch64.tar.gz',INTEL:'macos-x86_64.tar.gz',LINUX:'linux-x86_64-gnu.tar.gz',LINUX_ARM:'linux-aarch64-gnu.tar.gz'}
REVISIONS=['v1_0_8','v1_0_45','current_msvc']

def name(v,p): return f'grok-zh-{v}-{SUFFIX[p]}'
def asset(v,p,sidecar=False):
    n=name(v,p)+('.sha256' if sidecar else '')
    tag=f'v{v}' if v=='1.0.8' else f'release-v{v}'
    return {'name':n,'browser_download_url':f'https://github.com/{REPO}/releases/download/{tag}/{n}','size':128 if sidecar else 4096,'digest':'sha256:'+'a'*64,'state':'uploaded'}
def release(v,platforms=(GNU,),six=False):
    assets=[asset(v,p) for p in platforms]
    if six: assets=[asset(v,p,s) for p in (GNU,MAC,LINUX) for s in (False,True)]
    return {'tag_name':f'v{v}' if v=='1.0.8' else f'release-v{v}','body':'SYNTHETIC FIXTURE ONLY; never read from GitHub','draft':False,'prerelease':'-' in v,'immutable':True,'assets':assets}

def build():
    cases=[]
    def ok(revision,case,p,releases,v,selected=None,mode='latest',channel='stable'):
        cases.append({'id':revision+'__'+case+'__'+p.lower(),'revision':revision,'mode':mode,'platform':p,'channel':channel,'requested_version':v,'releases':copy.deepcopy(releases),'expect':{'version':v,'asset_name':name(v,selected or p)}})
    def error(revision,case,p,releases,substring='no immutable',v='1.0.99',mode='latest',channel='stable'):
        cases.append({'id':revision+'__'+case+'__'+p.lower(),'revision':revision,'mode':mode,'platform':p,'channel':channel,'requested_version':v,'releases':copy.deepcopy(releases),'expect':{'error_contains':substring}})
    bridge=release('1.0.45',six=True)
    later_gnu=release('1.0.99')
    later_native=release('1.0.100',(MSVC,))
    # The historical bridge inventory below is synthetic; live availability is deliberately out of scope.
    for revision in REVISIONS:
        for p in (GNU,MAC,LINUX):
            ok(revision,'frozen_six_asset_bridge',p,[bridge],'1.0.45')
        old=release('1.0.8');old['assets'].append(asset('1.0.8',GNU,True))
        ok(revision,'plain_v108_two_asset_bridge',GNU,[old],'1.0.8')
        for p in (MAC,LINUX): error(revision,'plain_bridge_no_unix',p,[old])
        ok(revision,'bridge_preserved_new_native_only_skipped',GNU,[later_native,bridge],'1.0.45')
        for p in (GNU,MAC,LINUX):
            expected='1.0.45' if revision=='v1_0_8' or p!=GNU else '1.0.99'
            ok(revision,'bridge_then_gnu_only_future',p,[later_native,later_gnu,bridge],expected)
        six_future=release('1.0.99',six=True)
        for p in (GNU,MAC,LINUX): ok(revision,'modern_exact_six_accepted',p,[six_future],'1.0.99')
        for kind in ('extra_native','missing_sidecar','corrupt_unrelated','duplicate_unrelated','new_platform_archives'):
            changed=copy.deepcopy(six_future)
            if kind=='extra_native': changed['assets'].append(asset('1.0.99',MSVC))
            elif kind=='missing_sidecar': changed['assets']=[a for a in changed['assets'] if not a['name'].endswith('macos-aarch64.tar.gz.sha256')]
            elif kind=='corrupt_unrelated': changed['assets'][2]['digest']='sha256:bad'
            elif kind=='duplicate_unrelated': changed['assets'].append(copy.deepcopy(changed['assets'][2]))
            else: changed['assets'] += [asset('1.0.99',p) for p in (ARM,INTEL,LINUX_ARM)]
            if revision=='v1_0_8': error(revision,kind, GNU,[changed], 'exact approved platform asset set' if kind!='corrupt_unrelated' else 'invalid SHA-256 digest',mode='asset')
            else: ok(revision,kind,GNU,[changed],'1.0.99',mode='asset')
        for kind in ('missing','duplicate','not_uploaded','zero_size','over_limit','wrong_repository','missing_digest','bad_digest'):
            changed=copy.deepcopy(six_future);a=changed['assets'][0]
            if kind=='missing':changed['assets'].pop(0)
            elif kind=='duplicate':changed['assets'].append(copy.deepcopy(a))
            elif kind=='not_uploaded':a['state']='new'
            elif kind=='zero_size':a['size']=0
            elif kind=='over_limit':a['size']=512*1024*1024+1
            elif kind=='wrong_repository':a['browser_download_url']=a['browser_download_url'].replace(REPO,'other/example')
            elif kind=='missing_digest':a.pop('digest')
            else:a['digest']='sha256:bad'
            error(revision,'selected_'+kind,GNU,[changed])
            ok(revision,'invalid_newer_skipped_'+kind,GNU,[changed,bridge],'1.0.45')
        for kind in ('draft','mutable','wrong_namespace','prerelease_mismatch','build_metadata'):
            changed=copy.deepcopy(six_future)
            if kind=='draft': changed['draft']=True
            elif kind=='mutable':changed['immutable']=False
            elif kind=='wrong_namespace':changed['tag_name']='v1.0.99'
            elif kind=='prerelease_mismatch':changed['prerelease']=True
            else:changed['tag_name']='release-v1.0.99+local'
            ok(revision,'invalid_release_skipped_'+kind,GNU,[changed,bridge],'1.0.45')
        alpha=release('1.0.100-rc.1',six=True)
        ok(revision,'stable_excludes_prerelease',GNU,[alpha,bridge],'1.0.45')
        ok(revision,'alpha_includes_prerelease',GNU,[bridge,alpha],'1.0.100-rc.1',channel='alpha')
        error(revision,'bad_channel',GNU,[bridge],'unsupported community release channel',channel='beta')
        error(revision,'no_releases',GNU,[])
        error(revision,'requested_version_mismatch',GNU,[bridge],'release tag and requested version do not match',v='1.0.99',mode='asset')
    # The exact MSVC selection branch distinguishes potential GNU fallback from verified extraction.
    ok('current_msvc','native_only_successor',MSVC,[bridge,later_gnu,later_native],'1.0.100')
    ok('current_msvc','gnu_only_potential_fallback',MSVC,[bridge,later_gnu],'1.0.99',selected=GNU)
    ok('current_msvc','native_prefers_same_version',MSVC,[release('1.0.99',(GNU,MSVC))],'1.0.99')
    ok('current_msvc','newer_gnu_beats_older_native',MSVC,[release('1.0.98',(MSVC,)),later_gnu],'1.0.99',selected=GNU)
    ok('current_msvc','native_nplus1_after_gnu_n',MSVC,[later_native,later_gnu],'1.0.100')
    # All metadata validity failures of a present native candidate fail closed, not GNU/version fallback.
    for kind in ('duplicate','not_uploaded','zero_size','over_limit','wrong_repository','missing_digest','bad_digest'):
        changed=release('1.0.99',(GNU,MSVC));a=changed['assets'][1]
        if kind=='duplicate':changed['assets'].append(copy.deepcopy(a));err='duplicated or not uploaded'
        elif kind=='not_uploaded':a['state']='new';err='duplicated or not uploaded'
        elif kind=='zero_size':a['size']=0;err='outside the accepted range'
        elif kind=='over_limit':a['size']=512*1024*1024+1;err='outside the accepted range'
        elif kind=='wrong_repository':a['browser_download_url']=a['browser_download_url'].replace(REPO,'other/example');err='fixed community repository'
        elif kind=='missing_digest':a.pop('digest');err='missing its GitHub SHA-256 digest'
        else:a['digest']='sha256:bad';err='invalid SHA-256 digest'
        error('current_msvc','invalid_native_fails_closed_'+kind,MSVC,[changed,release('1.0.98',(MSVC,))],err)
        ok('current_msvc','gnu_ignores_unrelated_invalid_native_'+kind,GNU,[changed],'1.0.99')
    invalid_gnu=release('1.0.99');invalid_gnu['assets'][0]['digest']='sha256:bad'
    error('current_msvc','invalid_newer_gnu_fails_closed',MSVC,[release('1.0.98',(MSVC,)),invalid_gnu],'invalid SHA-256 digest')
    invalid_unrelated=release('1.0.99',(GNU,MSVC));invalid_unrelated['assets'][0]['digest']='sha256:bad'
    ok('current_msvc','valid_native_ignores_invalid_gnu',MSVC,[invalid_unrelated],'1.0.99')
    for p in (GNU,MSVC,ARM,MAC,INTEL,LINUX,LINUX_ARM):
        ok('current_msvc','per_platform_asset',p,[release('1.0.99',tuple(SUFFIX))],'1.0.99')
        if p!=MSVC:error('current_msvc','native_x64_not_other_platform_fallback',p,[release('1.0.99',(MSVC,))])
    alpha=release('1.0.100-rc.1',(MSVC,))
    ok('current_msvc','msvc_stable_excludes_prerelease',MSVC,[alpha,later_gnu],'1.0.99',selected=GNU)
    ok('current_msvc','msvc_alpha_includes_prerelease',MSVC,[alpha,later_gnu],'1.0.100-rc.1',channel='alpha')
    # Exact set lookup is independent of API array order.
    shuffled=copy.deepcopy(bridge);shuffled['assets'].reverse()
    for revision in REVISIONS:ok(revision,'asset_order_irrelevant',GNU,[shuffled],'1.0.45')
    ids=[c['id'] for c in cases];assert len(ids)==len(set(ids))
    return {'schema':1,'fixture_kind':'SYNTHETIC_OFFLINE_API_METADATA','warning':'No release object here is evidence of a live GitHub release, immutable inventory, real bytes, digest, download, old published binary, activation, or runtime success. Digests and sizes are deliberately synthetic. GNU fallback selection is not extraction acceptance.','cases':cases}
if __name__=='__main__':
    out=ROOT/'fixtures/selection-matrix.json';out.write_text(json.dumps(build(),indent=2)+'\n');print(f'Wrote {len(build()["cases"])} synthetic cases')
