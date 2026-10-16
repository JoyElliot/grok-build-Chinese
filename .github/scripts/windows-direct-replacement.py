#!/usr/bin/env python3
"""Pinned real-binary direct-replacement experiment. Never installs or updates a product."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import shutil
import struct
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import zipfile

SCRIPT = Path(__file__).resolve()
INPUTS = SCRIPT.with_name('windows-direct-replacement-inputs.json')
MAX_ARCHIVE_BYTES = 200_000_000
MAX_UNPACKED_BYTES = 600_000_000


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(chunk)
    return h.hexdigest()


def verify_file(path, sha, size=None):
    require(Path(path).is_file(), f'Missing input: {path}')
    if size is not None:
        require(Path(path).stat().st_size == size, f'Unexpected size: {path}')
    require(digest(path) == sha, f'SHA-256 mismatch: {path}')


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None


def request(url, token=None):
    require(urllib.parse.urlsplit(url).scheme == 'https', 'HTTPS required')
    headers = {'User-Agent': 'grok-direct-replacement-verification', 'Accept': 'application/vnd.github+json'}
    if token:
        require(urllib.parse.urlsplit(url).netloc == 'api.github.com', 'Token only allowed at GitHub API')
        headers['Authorization'] = 'Bearer ' + token
    req = urllib.request.Request(url, headers=headers)
    opener = urllib.request.build_opener(NoRedirect())
    try:
        return opener.open(req, timeout=90)
    except urllib.error.HTTPError as exc:
        if exc.code not in (301, 302, 303, 307, 308):
            raise
        redirect = exc.headers['Location']
        parsed = urllib.parse.urlsplit(redirect)
        allowed = parsed.hostname in ('github.com', 'release-assets.githubusercontent.com') or (parsed.hostname or '').endswith('.blob.core.windows.net')
        require(parsed.scheme == 'https' and allowed, 'Unexpected GitHub download redirect host')
        # Never forward credentials to the artifact/release storage URL.
        return request(redirect)


def fetch_json(url, token):
    with request(url, token) as response:
        return json.load(response)


def download(url, destination, token=None):
    count = 0
    with request(url, token) as response, destination.open('xb') as target:
        while chunk := response.read(1024 * 1024):
            count += len(chunk)
            require(count <= MAX_ARCHIVE_BYTES, 'Download too large')
            target.write(chunk)


def acquire(inputs, output):
    require(not output.exists(), 'Download directory must not exist')
    output.mkdir(parents=True)
    api = 'https://api.github.com/repos/' + inputs['repository']
    token = os.environ.get('GH_TOKEN')
    old, native = inputs['historical'], inputs['native']
    release = fetch_json(api + '/releases/tags/' + old['tag'], token)
    require(release['id'] == old['release_id'] and release['immutable'] and not release['draft'] and not release['prerelease'], 'Historical release identity changed')
    asset = next(a for a in release['assets'] if a['id'] == old['asset_id'])
    require(asset['name'] == old['asset_name'] and asset['size'] == old['archive_bytes'] and asset['digest'] == 'sha256:' + old['archive_sha256'], 'Historical asset identity changed')
    ref = fetch_json(api + '/git/ref/tags/' + old['tag'], token)
    obj = ref['object']
    for _ in range(4):
        if obj['type'] != 'tag':
            break
        obj = fetch_json(api + '/git/tags/' + obj['sha'], token)['object']
    require(obj['type'] == 'commit' and obj['sha'] == old['source_commit'], 'Historical tag source changed')
    run = fetch_json(api + '/actions/runs/' + str(native['run_id']), token)
    require(run['head_sha'] == native['source_commit'] and run['head_branch'] == native['source_branch'] and run['run_attempt'] == native['run_attempt'], 'MSVC run source changed')
    require(run['status'] == 'completed' and run['conclusion'] == 'success' and run['path'] == native['workflow_path'] and run['repository']['id'] == inputs['repository_id'] and run['head_repository']['id'] == inputs['repository_id'], 'Untrusted or unsuccessful MSVC run')
    artifact = fetch_json(api + '/actions/artifacts/' + str(native['artifact_id']), token)
    require(not artifact['expired'] and artifact['name'] == native['artifact_name'] and artifact['size_in_bytes'] == native['artifact_bytes'] and artifact['digest'] == 'sha256:' + native['artifact_sha256'], 'MSVC artifact identity changed')
    binding = artifact['workflow_run']
    require(binding['id'] == native['run_id'] and binding['head_sha'] == native['source_commit'] and binding['repository_id'] == inputs['repository_id'] and binding['head_repository_id'] == inputs['repository_id'], 'MSVC artifact run binding changed')
    # Release download URL is independently fixed by repository, tag and name.
    expected = f"https://github.com/{inputs['repository']}/releases/download/{old['tag']}/{old['asset_name']}"
    require(asset['browser_download_url'] == expected, 'Unexpected historical download URL')
    download(expected, output / 'historical-gnu.zip')
    download(api + '/actions/artifacts/' + str(native['artifact_id']) + '/zip', output / 'native-msvc-artifact.zip', token)
    verify_file(output / 'historical-gnu.zip', old['archive_sha256'], old['archive_bytes'])
    verify_file(output / 'native-msvc-artifact.zip', native['artifact_sha256'], native['artifact_bytes'])
    # Do not record tokens or signed redirect URLs.
    (output / 'provenance.json').write_text(json.dumps({'inputs': inputs, 'release': {'id': release['id'], 'immutable': release['immutable'], 'tag_commit': obj['sha']}, 'run': {'id': run['id'], 'head_sha': run['head_sha'], 'run_attempt': run['run_attempt'], 'conclusion': run['conclusion']}, 'artifact': {'id': artifact['id'], 'digest': artifact['digest']}}, indent=2) + '\n', encoding='utf-8')


def extract_zip(archive, destination):
    require(not destination.exists(), 'Extraction destination must not exist')
    with zipfile.ZipFile(archive) as z:
        names = set()
        total = 0
        for item in z.infolist():
            # ZipInfo normalizes backslashes on Windows and truncates NULs.
            # Reject raw names that changed before applying our path policy.
            require(item.orig_filename == item.filename, 'Normalized ZIP path forbidden')
            p = PurePosixPath(item.filename)
            require(not p.is_absolute() and all(part not in ('', '.', '..') for part in p.parts) and '\\' not in item.filename and ':' not in item.filename, 'Unsafe ZIP path')
            require(not any(part.rstrip(' .') != part for part in p.parts), 'Unsafe Windows ZIP path')
            require(item.filename.casefold() not in names, 'Duplicate ZIP path')
            names.add(item.filename.casefold())
            mode = item.external_attr >> 16
            require((mode & 0o170000) != 0o120000, 'ZIP symlink forbidden')
            total += item.file_size
            require(total <= MAX_UNPACKED_BYTES, 'ZIP expansion too large')
        z.extractall(destination)


def package_root(root):
    matches = list(root.glob('*/grok-zh.exe'))
    require(len(matches) == 1, 'Expected exactly one top-level package executable')
    return matches[0].parent


def file_manifest(root):
    return {p.relative_to(root).as_posix(): digest(p) for p in sorted(root.rglob('*')) if p.is_file()}


def check_package(root, expected):
    verify_file(root / 'grok-zh.exe', expected['executable_sha256'])
    info = (root / 'BUILD-INFO.txt').read_text(encoding='utf-8-sig')
    for key, value in [('Commit', expected['source_commit']), ('Target', expected['target']), ('Version', expected['version'])]:
        require(f'{key}: {value}' in info.splitlines(), f'Incorrect {key} provenance')
    sums = (root / 'SHA256SUMS.txt').read_text(encoding='utf-8-sig')
    entries = {}
    for line in sums.splitlines():
        require(bool(re.fullmatch(r'[0-9a-fA-F]{64} [ *].+', line)), 'Invalid package manifest line')
        sha, name = line[:64].lower(), line[66:]
        name = name.replace('\\', '/')
        p = PurePosixPath(name)
        require(not p.is_absolute() and '..' not in p.parts and ':' not in name, 'Unsafe manifest path')
        require(name not in entries, 'Duplicate manifest file')
        entries[name] = sha
        verify_file(root / name, sha)
    require(entries.get('grok-zh.exe') == expected['executable_sha256'], 'Executable absent from package manifest')
    require((root / 'rg.exe').is_file() and (root / 'agent-zh.cmd').is_file(), 'Historical companion tools missing')


def inspect_pe(path):
    data = path.read_bytes()
    def u16(off): return struct.unpack_from('<H', data, off)[0]
    def u32(off): return struct.unpack_from('<I', data, off)[0]
    require(data[:2] == b'MZ' and len(data) >= 64, 'Not a PE executable')
    pe = u32(0x3c)
    require(data[pe:pe+4] == b'PE\0\0', 'Bad PE signature')
    optional = pe + 24
    require(u16(pe+4) == 0x8664 and u16(optional) == 0x20b, 'Expected x64 PE32+')
    count = u16(pe+6)
    sections = []
    start = optional + u16(pe+20)
    for i in range(count):
        off = start + i * 40
        sections.append({'name': data[off:off+8].split(b'\0')[0].decode('ascii'), 'virtual_size': u32(off+8), 'rva': u32(off+12), 'raw_size': u32(off+16), 'raw_offset': u32(off+20)})
    def offset(rva):
        for section in sections:
            if section['rva'] <= rva < section['rva'] + section['raw_size']:
                at = section['raw_offset'] + rva - section['rva']
                require(at < len(data), 'PE RVA outside file')
                return at
        raise ValueError(f'Unmapped PE RVA: {rva}')
    def cstring(rva):
        at = offset(rva)
        end = data.index(b'\0', at, min(at+512, len(data)))
        return data[at:end].decode('ascii').lower()
    def directory(index):
        require(u32(optional+108) > index, 'Missing PE data directory')
        return u32(optional+112+index*8), u32(optional+116+index*8)
    imports = []
    rva, size = directory(1)
    if rva:
        at = offset(rva)
        for n in range(size // 20):
            entry = struct.unpack_from('<5I', data, at+n*20)
            if not any(entry): break
            imports.append(cstring(entry[3]))
        else: raise ValueError('Unterminated import table')
    delay = []
    rva, size = directory(13)
    if rva:
        at = offset(rva)
        for n in range(size // 32):
            entry = struct.unpack_from('<8I', data, at+n*32)
            if not any(entry): break
            require(entry[0] & 1, 'VA-based delay imports unsupported')
            delay.append(cstring(entry[1]))
        else: raise ValueError('Unterminated delay import table')
    resource_rva, resource_size = directory(2)
    certificate_offset, certificate_size = directory(4)
    deps = sorted(set(imports + delay))
    return {'sha256': digest(path), 'bytes': len(data), 'machine': 'x64', 'format': 'PE32+', 'subsystem': u16(optional+68), 'subsystem_version': [u16(optional+48), u16(optional+50)], 'imports': sorted(set(imports)), 'delay_imports': sorted(set(delay)), 'msvc_redistributable_imports': [x for x in deps if x.startswith(('vcruntime', 'msvcp', 'concrt'))], 'resources': {'rva': resource_rva, 'bytes': resource_size}, 'certificate_table': {'file_offset': certificate_offset, 'bytes': certificate_size}, 'sections': sections}


def isolated_environment(root):
    # Never inherit credentials, developer tool directories, user config or PATH.
    keep = ('SystemRoot', 'WINDIR', 'COMSPEC', 'PATHEXT', 'SystemDrive', 'OS', 'PROCESSOR_ARCHITECTURE', 'NUMBER_OF_PROCESSORS')
    env = {key: os.environ[key] for key in keep if key in os.environ}
    system = Path(os.environ['SystemRoot'])
    env['PATH'] = str(system / 'System32') + os.pathsep + str(system)
    for name, suffix in [('HOME', 'home'), ('USERPROFILE', 'home'), ('APPDATA', 'home/AppData/Roaming'), ('LOCALAPPDATA', 'home/AppData/Local'), ('TEMP', 'temp'), ('TMP', 'temp'), ('GROK_HOME', 'home/.grok-direct-test')]:
        dest = root / suffix
        dest.mkdir(parents=True, exist_ok=True)
        env[name] = str(dest)
    # CLI help/version terminate before any update, authentication or provider call.
    return env


class UnconfirmedProcessTermination(RuntimeError):
    """A bounded cleanup ended without proving the product process tree stopped."""


def run_command(command, cwd, env, label, output, expected_version=None, timeout_seconds=35, require_stdout=True):
    started = time.monotonic()
    stdout_path, stderr_path = output / (label + '.stdout.txt'), output / (label + '.stderr.txt')
    # Direct file handles avoid pipe-reader threads and unbounded descendant pipe drains.
    with stdout_path.open('wb') as stdout_file, stderr_path.open('wb') as stderr_file:
        process = subprocess.Popen(command, cwd=cwd, env=env, stdin=subprocess.DEVNULL, stdout=stdout_file, stderr=stderr_file)
        try:
            process.wait(timeout=timeout_seconds)
        except subprocess.TimeoutExpired as exc:
            taskkill = str(Path(env['SystemRoot']) / 'System32/taskkill.exe')
            cleanup = {'tree_termination': 'attempted', 'returncode': None, 'error': None}
            try:
                killed = subprocess.run([taskkill, '/PID', str(process.pid), '/T', '/F'], stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10, check=False)
                cleanup['returncode'] = killed.returncode
                cleanup['output'] = (killed.stdout + killed.stderr).decode('utf-8', errors='replace')
            except (subprocess.TimeoutExpired, OSError) as cleanup_error:
                cleanup['error'] = str(cleanup_error)
            finally:
                if process.poll() is None:
                    try:
                        process.kill()
                    except OSError as kill_error:
                        cleanup['root_kill_error'] = str(kill_error)
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    cleanup['root_wait'] = 'timed_out'
                cleanup['root_termination_confirmed'] = process.poll() is not None
                cleanup['tree_termination_confirmed'] = cleanup['returncode'] == 0 and cleanup['error'] is None
                cleanup_log_error = None
                try:
                    (output / (label + '.cleanup.txt')).write_text(json.dumps(cleanup, indent=2) + '\n', encoding='utf-8')
                except OSError as error:
                    cleanup_log_error = error
            if not cleanup['root_termination_confirmed'] or not cleanup['tree_termination_confirmed']:
                raise UnconfirmedProcessTermination(f'{label}: process tree termination could not be confirmed') from (cleanup_log_error or exc)
            if cleanup_log_error is not None:
                raise cleanup_log_error
            raise ValueError(f'{label} exceeded {timeout_seconds} seconds') from exc
    elapsed = round(time.monotonic() - started, 3)
    stdout, stderr = stdout_path.read_bytes(), stderr_path.read_bytes()
    require(process.returncode == 0, f'{label} failed, exit={process.returncode}')
    if require_stdout:
        require(bool(stdout.strip()), f'{label} has empty stdout')
    if expected_version:
        text = stdout.decode('utf-8', errors='replace')
        require(bool(re.search(r'^grok-zh ' + re.escape(expected_version) + r'\s*\(', text, re.MULTILINE)), f'{label}: unexpected product version: {text[:200]}')
    return {'label': label, 'arguments': command[1:], 'returncode': process.returncode, 'seconds': elapsed, 'timeout_seconds': timeout_seconds, 'stdout_sha256': hashlib.sha256(stdout).hexdigest(), 'stderr_sha256': hashlib.sha256(stderr).hexdigest()}


def validate(inputs, archives, output, static_only):
    require(not output.exists(), 'Result directory must not exist')
    output.mkdir(parents=True)
    result = {'schema': 1, 'status': 'running', 'inputs': inputs, 'windows_execution': 'not_run', 'scope': 'real historical archive; top-level EXE-only replacement in a fresh copy', 'limitations': ['No live release network update chain', 'No subsequent MSVC self-update', 'No TUI, real account or provider invocation', 'Historical launcher and installer scripts are preserved but not executed', 'No locked-file replacement, interruption, rollback, low-space or AV acceptance test', 'PE imports do not enumerate runtime LoadLibrary calls; hosted Windows is not a pristine end-user machine'], 'commands': []}
    try:
        old, native = inputs['historical'], inputs['native']
        old_zip = archives / 'historical-gnu.zip'
        outer_zip = archives / 'native-msvc-artifact.zip'
        verify_file(old_zip, old['archive_sha256'], old['archive_bytes'])
        verify_file(outer_zip, native['artifact_sha256'], native['artifact_bytes'])
        extract_zip(outer_zip, output / 'outer')
        native_zip = output / 'outer' / native['archive_name']
        verify_file(native_zip, native['archive_sha256'], native['archive_bytes'])
        outer_files = {p.name for p in (output / 'outer').iterdir()}
        require(outer_files == {native['archive_name'], native['archive_name'] + '.sha256'}, 'Unexpected standalone artifact contents')
        extract_zip(old_zip, output / 'historical')
        extract_zip(native_zip, output / 'native')
        old_root, native_root = package_root(output / 'historical'), package_root(output / 'native')
        check_package(old_root, old)
        check_package(native_root, native)
        result['pe'] = {'historical': inspect_pe(old_root / 'grok-zh.exe'), 'native': inspect_pe(native_root / 'grok-zh.exe')}
        require(not result['pe']['native']['msvc_redistributable_imports'], 'MSVC redistributable import requires further review')
        require(result['pe']['native']['subsystem'] == 3, 'Expected console subsystem')
        pristine = file_manifest(old_root)
        if not static_only:
            require(sys.platform == 'win32', 'Real execution requires Windows; use --static-only elsewhere')
            isolated = output / 'isolated-user'
            env = isolated_environment(isolated)
            logs = output / 'logs'
            logs.mkdir()
            result['windows_execution'] = 'running'
            for label, args in [('version', ['--version']), ('help', ['--help']), ('agent-help', ['agent', '--help']), ('update-help', ['update', '--help'])]:
                result['commands'].append(run_command([str(old_root / 'grok-zh.exe'), *args], old_root, env, 'historical-' + label, logs, old['version'] if label == 'version' else None))
            require(file_manifest(old_root) == pristine, 'Historical package was modified by baseline smoke')
        replacement = output / 'replacement'
        shutil.copytree(old_root, replacement)
        shutil.copyfile(native_root / 'grok-zh.exe', replacement / 'grok-zh.exe')
        actual = file_manifest(replacement)
        changed = sorted(name for name in pristine.keys() | actual.keys() if pristine.get(name) != actual.get(name))
        require(changed == ['grok-zh.exe'], f'Unexpected changed files: {changed}')
        result['replacement'] = {'changed_files': changed, 'preserved_file_count': len(pristine)-1, 'old_manifest': pristine, 'replaced_manifest': actual, 'retained_metadata_is_stale_by_design': True}
        # In particular, do not execute any old/new install or migration script.
        forbidden = [p.relative_to(output).as_posix() for p in output.rglob('*') if '.grok-zh-msvc' in p.parts or 'bootstrap' in p.name.lower() or 'migration' in p.name.lower()]
        require(not forbidden, f'Unexpected migration file: {forbidden}')
        if static_only:
            result['status'] = 'static_pass_windows_pending'
            return
        for label, args in [('version', ['--version']), ('help', ['--help']), ('agent-help', ['agent', '--help']), ('update-help', ['update', '--help'])]:
            result['commands'].append(run_command([str(replacement / 'grok-zh.exe'), *args], replacement, env, 'direct-' + label, logs, native['version'] if label == 'version' else None))
        result['commands'].append(run_command([str(replacement / 'rg.exe'), '--version'], replacement, env, 'preserved-rg-version', logs))
        # Historical launcher/installer scripts are preserved and hashed, never executed.
        require(file_manifest(old_root) == pristine, 'Historical source package was modified')
        require(file_manifest(replacement) == actual, 'Smoke commands changed the package')
        generated = [p.relative_to(isolated).as_posix() for p in isolated.rglob('*') if p.is_file()]
        result['isolated_user_files'] = generated
        forbidden = [p for p in generated if '.grok-zh-msvc' in p or 'migration' in p.lower() or 'bootstrap' in p.lower() or p.lower().endswith(('.ps1', '.exe', '.dll'))]
        require(not forbidden, f'Unexpected runtime/bootstrap/migration files: {forbidden}')
        result['windows_execution'] = 'passed'
        result['status'] = 'pass'
    except Exception as exc:
        result['status'] = 'fail'
        if result['windows_execution'] == 'running':
            result['windows_execution'] = 'failed'
        result['error'] = str(exc)
        raise
    finally:
        (output / 'RESULT.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--inputs', type=Path, default=INPUTS)
    p.add_argument('--archives', type=Path, required=True)
    p.add_argument('--output', type=Path)
    p.add_argument('--download', action='store_true')
    p.add_argument('--static-only', action='store_true')
    args = p.parse_args()
    inputs = json.loads(args.inputs.read_text(encoding='utf-8'))
    if args.download:
        acquire(inputs, args.archives.resolve())
    else:
        require(args.output is not None, '--output required')
        validate(inputs, args.archives.resolve(), args.output.resolve(), args.static_only)


if __name__ == '__main__':
    main()
