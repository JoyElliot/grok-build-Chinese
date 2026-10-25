#!/usr/bin/env python3
"""Compile original updater bytes, with declared fixture transport/destination seams.
Reuses the exact-function extraction design of 3a1359d historical-update-parser.
Never patches executable versions, networking policy or production source files.
"""
import hashlib, json, pathlib, re
BASE = pathlib.Path(__file__).resolve().parent
ROOT = BASE.parents[2]
OUT = BASE / 'generated'
PINS = json.loads((BASE / 'historical-pins.json').read_text())
report = {'historical_commit': PINS['commit'], 'sources': [], 'slices': [], 'seams': [
    'Explicit fixture metadata replaces GitHub discovery and response authenticity',
    'Explicit platform selection: GNU for historical, MSVC for current',
    'Local 64-KiB archive stream replaces download_verified HTTPS adapter only',
    'Explicit isolated package-copy destination replaces current_exe',
    'Production entry policy and success receipt/UI/cache dispatch excluded',
    'Current ExplicitVersion intent supplied by harness, not original CLI intent selection',
    'Both harnesses use current pinned toolchain / MSVC ABI, not historical GNU product ABI',
    'Current same-version reinstall only; no distinct future version claim',
    'Current process-kill tests run copied harness EXEs, not product binaries; cfg(test) checkpoints bracket exact production replacement',
    'Pre-fix b3ca1daa activation negative fixture is hash-verified; only a test checkpoint after first rename is inserted',
    'Legacy delayed-writer negative case invokes exact historical activation with an explicit previously validated candidate fixture, not whole immutable old updater',
]}

def read(path, historical=False):
    source = (BASE / 'historical' if historical else ROOT) / path
    data = source.read_bytes()
    blob = hashlib.sha1(f'blob {len(data)}\0'.encode() + data).hexdigest()
    if historical and blob != PINS['git_blobs'][path]:
        raise RuntimeError(f'Historical source changed: {path}')
    report['sources'].append({'path': path, 'historical': historical, 'git_blob_sha1': blob, 'sha256': hashlib.sha256(data).hexdigest()})
    return data

def write(path, data):
    target = OUT / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_bytes(data)

def exact_item(data, name, mode, kind='fn', derive=False):
    pattern = rb'^(?:pub(?:\([^\n]*?\))? )?(?:async )?' + kind.encode() + b' ' + name.encode() + rb'\b'
    match = re.search(pattern, data, re.M)
    if not match:
        raise RuntimeError(f'Missing {mode} {kind} {name}')
    start = match.start()
    if derive:
        previous = data.rfind(b'#[derive(', 0, start)
        if previous < 0 or data[previous:start].count(b'\n') > 1:
            raise RuntimeError(f'Missing derive for {name}')
        start = previous
    end = re.search(rb'^}', data[match.end():], re.M)
    if not end:
        raise RuntimeError(f'Missing end of {name}')
    stop = match.end() + end.end()
    value = data[start:stop] + b'\n'
    report['slices'].append({'mode': mode, 'item': name, 'first_line':data[:start].count(b'\n')+1, 'last_line':data[:stop].count(b'\n')+1, 'sha256':hashlib.sha256(value).hexdigest()})
    return b'\n// Verbatim source item: ' + name.encode() + b'\n' + value

def between(data, start, end, mode, label):
    # Match checkout line endings without changing any extracted source byte.
    def locate(needle, offset=0):
        pattern = rb'\r?\n'.join(re.escape(part) for part in needle.split(b'\n'))
        match = re.search(pattern, data[offset:])
        if match is None:
            raise RuntimeError(f'Missing source anchor for {mode}: {label}')
        return offset + match.start()
    first = locate(start); last = locate(end, first)
    value = data[first:last]
    report['slices'].append({'mode':mode, 'item':label, 'first_line':data[:first].count(b'\n')+1, 'last_line':data[:last].count(b'\n')+1, 'sha256':hashlib.sha256(value).hexdigest()})
    return value

def main():
    for mode in ['legacy', 'current']:
        old = mode == 'legacy'
        src = 'crates/codegen/xai-grok-update/src/'
        release = read(src+'community_release.rs', old)
        release += b'\n#[cfg(all(windows, test))]\n#[path = "transaction.rs"]\nmod transaction;\n'
        write(mode+'/community_release.rs', release)
        paths = ['package_protocol.rs'] if old else [p.name for p in (ROOT/src/'community_release').glob('*.rs')]
        for name in paths:
            content = read(src+'community_release/'+name,old)
            # Plain `mod child` and explicit #[path] nested declarations differ.
            write(mode+'/'+name, content)
            write(mode+'/community_release/'+name, content)
        write(mode+'/community_update_cancel.rs',read(src+'community_update_cancel.rs',old))
        write(mode+'/product.rs',read('crates/codegen/xai-grok-product/src/lib.rs',old))
        auto = read(src+'auto_update.rs',old)
        tty = read('crates/codegen/xai-tty-utils/src/lib.rs',old)
        tx = b'// Generated original transaction items; seams are explicitly identified below.\nuse super::*;\n'
        if not old:
            tx += b'use std::io;\n'
        for name in ['SMOKE_TEST_TIMEOUT','SMOKE_TEST_ETXTBSY_ATTEMPTS','SMOKE_TEST_ETXTBSY_BACKOFF']:
            tx += re.search(rb'^(?:pub\(crate\) )?const '+name.encode()+rb':[^\n]+\n',auto,re.M).group(0)
        tx += exact_item(auto,'SmokeTestFailure',mode,'enum',True)
        tx += exact_item(auto,'VersionProbeError',mode,'enum')
        for name in ['unique_temp_sibling','truncate_err','nonzero_message','probe_version','smoke_test_binary','community_version_output_matches','windows_replace_exe','windows_replace_exe_locked','sweep_old_exe_backups','acquire_windows_update_lock']:
            tx += exact_item(auto,name,mode)
        if not old:
            tx += exact_item(auto,'InstallIntent',mode,'enum',True)
            for name in ['installed_community_version','community_windows_version_to_retain','activate_community_windows_candidate_locked','windows_wide_path','windows_rename_no_replace','windows_atomic_replace','windows_activate_staged_exe','windows_activation_test_checkpoint']:
                tx += exact_item(auto,name,mode)
        if not old:
            original = (BASE/'regressions/pre-fix-activation-b3ca1daa.rs').read_bytes()
            pin = json.loads((BASE/'regressions/pre-fix-activation-b3ca1daa.json').read_text())
            if hashlib.sha256(original).hexdigest() != pin['sha256']:
                raise RuntimeError('Pre-fix activation regression source changed')
            anchor = b'                renamed = true;'
            if original.count(anchor) != 1:
                raise RuntimeError('Pre-fix activation checkpoint anchor changed')
            instrumented = original.replace(anchor, anchor + b'\n                windows_activation_test_checkpoint("after-backup", dest);')
            report['slices'].append({'mode':'pre-fix-negative', 'item':'windows_activate_staged_exe', 'commit':pin['commit'], 'sha256':pin['sha256'], 'seam':'checkpoint inserted after first rename'})
            tx += b'\nmod pre_fix_activation { use super::*;\n' + instrumented
            tx += b'\npub(super) async fn activate(stage: &Path, dest: &Path, old: &Path) -> Result<()> { windows_activate_staged_exe(stage, dest, old).await }\n}\n'
        tx += b'\n// Namespace wiring only; original detach implementation.\nmod xai_grok_tools { pub mod util {\n'
        tx += exact_item(tty,'detach_command',mode)+b'\n} }\n'
        tx += b'\n// TEST SEAMS: fixture asset + isolated destination + local response stream.\nasync fn isolated_transaction(asset: VerifiedAsset, destination: std::path::PathBuf, source: &Path, truncate_stream: bool) -> Result<String> {\nlet version = asset.version.clone();\n'
        if not old:
            tx += b'let parsed = Version::parse(&version)?;\nlet _intent = InstallIntent::ExplicitVersion; // Explicit same-version reinstall, not an automatic upgrade.\n'
        tx += between(auto,b'        let destination_name = destination\n',b'        crate::community_release::download_verified(&asset, &archive).await?;',mode,'destination name guard and temporary paths')
        tx += b'\n// Only the HTTPS adapter call is replaced. Original stream validator follows.\nwrite_verified_download(&asset, &archive, &ProgressBar::hidden(), file_stream(source, truncate_stream)?).await?;\n'
        tx += between(auto,b'        let archive_for_extract = archive.clone();',b'        crate::community_update_notes::record_success(&version, asset.release_notes.as_deref());',mode,'extraction, real probe, lock and activation')
        tx += b'\n// Success receipt/UI deliberately outside isolated harness.\nOk(version)\n}\n'
        tx += f'\nconst LEGACY: bool = {str(old).lower()};\n'.encode()
        platform = 'WindowsX86_64Gnu' if old else 'WindowsX86_64Msvc'
        tx += f'fn fixture_platform() -> CommunityPlatform {{ CommunityPlatform::{platform} }}\n'.encode()
        tx += b'include!("../../transaction_tests.rs");\n'
        if not old:
            tx += b'include!("../../current_tests.rs");\n'
            tx += b'include!("../../atomic_activation_tests.rs");\n'
            unit_tests = read(src+'auto_update_tests.rs')
            tx += b'\nmod atomic_unit_tests { use super::*;\n'
            for name, asynchronous in [
                ('test_windows_rename_no_replace_preserves_existing_names', False),
                ('test_windows_missing_stage_preserves_destination_and_locked_backup', True),
                ('test_windows_stage_cleanup_when_destination_cannot_be_renamed', True),
                ('test_windows_replace_exe_supports_long_installation_paths', True),
                ('test_windows_atomic_replace_failure_keeps_public_entry_and_stage', True),
                ('test_windows_atomic_replace_rejects_nul_destination_without_mutation', False),
            ]:
                tx += b'\n#[tokio::test]\n' if asynchronous else b'\n#[test]\n'
                tx += exact_item(unit_tests, name, mode)
            tx += b'\n}\n'
        else:
            tx += b'include!("../../legacy_race_tests.rs");\n'
        write(mode+'/transaction.rs',tx)
    (BASE/'results').mkdir(exist_ok=True)
    (BASE/'results/source-provenance.json').write_text(json.dumps(report,indent=2)+'\n')
    print('Verified exact historical blobs and generated two independent transaction executables')

if __name__ == '__main__':
    main()
