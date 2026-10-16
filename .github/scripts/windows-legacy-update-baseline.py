#!/usr/bin/env python3
"""Genuine historical updater self-reinstall baseline: GNU -> SAME GNU, not MSVC."""
import argparse
import datetime
import importlib.util
import json
from pathlib import Path
import re
import shutil
import sys

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('direct', HERE / 'windows-direct-replacement.py')
direct = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(direct)


def file_identity(path):
    # Python 3.12 exposes the Windows file index in st_ino. A zero value is
    # insufficient evidence and will fail closed rather than count a no-op.
    # https://docs.python.org/3.12/library/os.html#os.stat_result
    info = path.stat()
    direct.require(info.st_ino != 0, 'Windows file identity unavailable')
    return {'volume': info.st_dev, 'file_index': str(info.st_ino)}


def verify_reinstall_evidence(before, after, identity_before, identity_after, old_hash, stderr):
    direct.require(all(after.get(name) == value for name, value in before.items()), 'Pinned reinstall changed original package content')
    direct.require(identity_before != identity_after, 'No actual EXE file-object replacement was observed')
    extras = sorted(after.keys() - before.keys())
    allowed = re.compile(r'grok-zh\.exe\.(?:update\.lock|old(?:\.\d+-\d+\.old)?)\Z')
    direct.require(all(allowed.fullmatch(name) for name in extras), 'Unexpected baseline package files: ' + repr(extras))
    backups = [name for name in extras if name.startswith('grok-zh.exe.old')]
    direct.require(bool(backups), 'No live-image rename-aside backup was retained')
    direct.require(all(after[name] == old_hash for name in backups), 'Retained backup differs from genuine old EXE')
    direct.require('已从 JoyElliot/grok-build-Chinese/releases 安装 grok-zh v1.0.45' in stderr, 'Missing historical transaction completion output')
    return backups


def baseline(validated_root, archives, output):
    direct.require(sys.platform == 'win32', 'Genuine historical baseline requires Windows')
    direct.require(not output.exists(), 'Baseline output must not exist')
    inputs = json.loads(direct.INPUTS.read_text(encoding='utf-8'))
    old = inputs['historical']
    prior = json.loads((validated_root / 'RESULT.json').read_text(encoding='utf-8'))
    provenance = json.loads((archives / 'provenance.json').read_text(encoding='utf-8'))
    direct.require(prior['status'] == 'pass' and prior['inputs'] == inputs and provenance['inputs'] == inputs,
                   'Matching successful real-binary validation and download provenance are required')
    direct.require(provenance['release'] == {'id': old['release_id'], 'immutable': True, 'tag_commit': old['source_commit']},
                   'Historical immutable source provenance mismatch')
    direct.verify_file(archives / 'historical-gnu.zip', old['archive_sha256'], old['archive_bytes'])
    source = direct.package_root(validated_root / 'historical')
    direct.check_package(source, old)
    source_before = direct.file_manifest(source)
    output.mkdir(parents=True)
    package = output / 'package'
    shutil.copytree(source, package)
    logs = output / 'logs'
    logs.mkdir()
    env = direct.isolated_environment(output / 'isolated-user')
    report = {'schema': 1, 'status': 'running', 'scope': 'Genuine GNU 1.0.45 -> SAME GNU 1.0.45 live self-reinstall',
              'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'msvc_compatibility_proven': False, 'historical_input': old,
              'network': 'Unmodified binary uses its original public GitHub HTTPS release endpoints, with no authentication or interception',
              'limitations': ['Not GNU-to-MSVC acceptance', 'Not subsequent MSVC update', 'Not proof of future release availability',
                              'No wire capture; API/download validation is attributed to the pinned original updater source and observed activation'],
              'commands': []}
    try:
        exe = package / 'grok-zh.exe'
        before = direct.file_manifest(package)
        report['before_manifest'] = before
        report['before_identity'] = file_identity(exe)
        report['commands'].append(direct.run_command([str(exe), '--version'], package, env, 'before-version', logs, old['version']))
        # The original run_update Some(version) branch invokes installation
        # directly, without needs_update's same-version no-op decision.
        report['commands'].append(direct.run_command([str(exe), 'update', '--version', old['version'], '--force-reinstall'],
                                                     package, env, 'same-version-update', logs, timeout_seconds=300, require_stdout=False))
        report['after_manifest'] = direct.file_manifest(package)
        report['after_identity'] = file_identity(exe)
        stderr = (logs / 'same-version-update.stderr.txt').read_text(encoding='utf-8', errors='replace')
        report['retained_old_image_backups'] = verify_reinstall_evidence(before, report['after_manifest'], report['before_identity'],
                                                                       report['after_identity'], old['executable_sha256'], stderr)
        report['retained_backup_identities'] = {name: file_identity(package / name) for name in report['retained_old_image_backups']}
        direct.require(report['before_identity'] in report['retained_backup_identities'].values(),
                       'Retained backup is not the original running EXE file object')
        report['commands'].append(direct.run_command([str(exe), '--version'], package, env, 'after-version', logs, old['version']))
        direct.verify_file(exe, old['executable_sha256'])
        direct.require(direct.file_manifest(source) == source_before, 'Original source package was modified')
        report['isolated_user_files'] = sorted(p.relative_to(output / 'isolated-user').as_posix()
                                              for p in (output / 'isolated-user').rglob('*') if p.is_file())
        report['status'] = 'passed_gnu_same_version_network_baseline'
    except Exception as error:
        report['status'] = 'failed_or_unavailable_baseline'
        report['error'] = str(error)
        raise
    finally:
        (output / 'BASELINE.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
        print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--validated-root', type=Path, required=True)
    parser.add_argument('--archives', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    baseline(args.validated_root.resolve(), args.archives.resolve(), args.output.resolve())
