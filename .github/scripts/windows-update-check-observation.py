#!/usr/bin/env python3
"""Read-only live discovery observation; never an install or compatibility gate."""
import argparse
import datetime
import importlib.util
import json
from pathlib import Path
import sys

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location('direct', HERE / 'windows-direct-replacement.py')
direct = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(direct)


def parse_status(text, expected_version):
    status = json.loads(text)
    direct.require(isinstance(status, dict), 'Update status is not an object')
    direct.require(status.get('currentVersion') == expected_version, 'Update status has unexpected currentVersion')
    direct.require(isinstance(status.get('updateAvailable'), bool), 'Update status lacks boolean updateAvailable')
    direct.require(status.get('latestVersion') is None or isinstance(status['latestVersion'], str), 'Invalid latestVersion')
    direct.require(status.get('error') is None or isinstance(status['error'], str), 'Invalid error field')
    outcome = 'observed' if status.get('error') is None and status.get('latestVersion') else 'unavailable'
    return outcome, status


def profile_files(profile):
    files = sorted(p.relative_to(profile).as_posix() for p in profile.rglob('*') if p.is_file())
    forbidden = [name for name in files if '.grok-zh-msvc' in name.lower() or 'bootstrap' in name.lower()
                 or 'migration' in name.lower() or name.lower().endswith(('.exe', '.dll', '.ps1', '.bat', '.cmd',
                     '.msi', '.msix', '.zip', '.tar.gz', '.tgz', '.7z', '.rar', '.sh'))]
    direct.require(not forbidden, 'Read-only observation created executable/installer/archive files: ' + repr(forbidden))
    return files


def observe(validated_root, output):
    direct.require(sys.platform == 'win32', 'Live binary observations require Windows')
    direct.require(not output.exists(), 'Observation directory must not exist')
    prior = json.loads((validated_root / 'RESULT.json').read_text(encoding='utf-8'))
    inputs = json.loads(direct.INPUTS.read_text(encoding='utf-8'))
    direct.require(prior['status'] == 'pass' and prior['inputs'] == inputs, 'A matching successful real-binary run is required')
    output.mkdir(parents=True)
    logs = output / 'logs'
    logs.mkdir()
    report = {'schema': 1, 'status': 'running', 'observed_at_utc': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'scope': 'Unmodified real binaries, update --check --json, existing public GitHub endpoint',
              'compatibility_gate': False, 'observations': [],
              'limitations': ['Time-sensitive live release discovery only', 'No asset download or installation',
                              'No unpublished direct-MSVC transport acceptance', 'No successful subsequent update is implied',
                              'Network/API errors remain unavailable observations, not compatibility conclusions']}
    try:
        packages = [('historical', direct.package_root(validated_root / 'historical'), inputs['historical']),
                    ('native', validated_root / 'replacement', inputs['native'])]
        for label, package, pin in packages:
            exe = package / 'grok-zh.exe'
            direct.verify_file(exe, pin['executable_sha256'])
            before = direct.file_manifest(package)
            profile = output / ('isolated-' + label)
            env = direct.isolated_environment(profile)
            record = {'binary': label, 'executable_sha256': pin['executable_sha256'], 'outcome': 'unavailable'}
            try:
                record['command'] = direct.run_command([str(exe), 'update', '--check', '--json'], package, env,
                                                       label + '-update-check', logs, timeout_seconds=90)
                text = (logs / (label + '-update-check.stdout.txt')).read_text(encoding='utf-8')
                record['outcome'], record['update_status'] = parse_status(text, pin['version'])
            except (ValueError, OSError) as error:
                record['observation_error'] = str(error)
            # UnconfirmedProcessTermination is deliberately NOT caught as an
            # unavailable network observation: inability to stop is a gate failure.
            # Integrity failures are real harness failures, independent of external availability.
            direct.require(direct.file_manifest(package) == before, 'Read-only check changed package files: ' + label)
            record['package_unchanged'] = True
            record['isolated_profile_files'] = profile_files(profile)
            report['observations'].append(record)
        report['status'] = 'observation_complete'
    except Exception as error:
        report['status'] = 'harness_error'
        report['error'] = str(error)
        raise
    finally:
        (output / 'OBSERVATION.json').write_text(json.dumps(report, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
        print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--validated-root', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    observe(args.validated_root.resolve(), args.output.resolve())
