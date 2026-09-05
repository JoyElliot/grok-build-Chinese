"""Test immutable macOS releases without compiling or using an xAI account.

This intentionally uses the production stable endpoint. If stable has moved on,
fail instead of silently testing a different upgrade or rewriting release data.
Only terminal capability replies are sent during the automatic-update case.
"""

import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import subprocess
import sys
import tarfile
import tempfile
import time
import traceback
import urllib.error
import urllib.request


REPO = "JoyElliot/grok-build-Chinese"
OLD = "1.0.12-rc.1"
NEW = "1.0.13"
RESULTS = []
EVIDENCE = Path(os.environ["RUNNER_TEMP"]) / "macos-release-update-evidence"
EVIDENCE.mkdir(exist_ok=False)
ROOT = Path(tempfile.mkdtemp(prefix="grok-macos-update-", dir=os.environ["RUNNER_TEMP"])).resolve()


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def digest(path):
    result = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            result.update(chunk)
    return result.hexdigest()


def command(args, *, env=None, cwd=None, timeout=180, log=None):
    result = subprocess.run(args, env=env, cwd=cwd, timeout=timeout,
                            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    if log:
        (EVIDENCE / log).write_text(result.stdout, encoding="utf-8")
    require(result.returncode == 0, f"Command failed ({result.returncode}): {args[0]}; {result.stdout[-3000:]}")
    return result.stdout


def api(endpoint):
    return json.loads(command(["gh", "api", f"repos/{REPO}/{endpoint}"]))


def check_runner_api_budget():
    headers = {"Accept": "application/vnd.github+json", "User-Agent": "grok-build-zh-updater",
               "X-GitHub-Api-Version": "2026-03-10"}
    deadline = time.monotonic() + 65 * 60
    history = []
    while True:
        request = urllib.request.Request("https://api.github.com/rate_limit", headers=headers)
        with urllib.request.urlopen(request, timeout=30) as response:
            core = json.load(response)["resources"]["core"]
        history.append({"observed_at": time.time(), **core})
        (EVIDENCE / "runner-api-budget.json").write_text(json.dumps(history, indent=2), encoding="utf-8")
        print(f"Runner anonymous GitHub API budget: {json.dumps(core)}", flush=True)
        if core["remaining"] >= 25:
            break
        delay = max(60, core["reset"] - time.time() + 5)
        require(time.monotonic() + delay < deadline,
                "Runner API budget did not recover within 65 minutes; environment prerequisite failed")
        print(f"Respecting GitHub rate-limit reset; waiting {round(delay)} seconds before retry", flush=True)
        resume = time.monotonic() + delay
        while time.monotonic() < resume:
            time.sleep(min(60, resume - time.monotonic()))
    request = urllib.request.Request(
        f"https://api.github.com/repos/{REPO}/releases?per_page=100&page=1", headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=30) as response:
            response.read()
    except urllib.error.HTTPError as error:
        evidence = {"status": error.code, "body": error.read().decode("utf-8", errors="replace"),
                    "rate_limit_headers": {key: value for key, value in error.headers.items()
                                           if key.lower().startswith(("x-ratelimit-", "retry-after"))}}
        (EVIDENCE / "runner-api-error.json").write_text(json.dumps(evidence, indent=2), encoding="utf-8")
        raise RuntimeError(f"Unauthenticated update feed is unavailable: {evidence}") from error


def assert_stable():
    latest = api("releases/latest")
    require(latest["tag_name"] == f"release-v{NEW}",
            f"Stable changed to {latest['tag_name']}; this test requires the real {NEW} stable feed")
    require(latest.get("immutable") and not latest["prerelease"] and not latest["draft"],
            "Target must be an immutable stable release")


def package(version):
    release = api(f"releases/tags/release-v{version}")
    require(release.get("immutable") and not release["draft"], "Source release must be immutable")
    (EVIDENCE / f"release-{version}.json").write_text(json.dumps(release, indent=2), encoding="utf-8")
    name = f"grok-zh-{version}-macos-aarch64.tar.gz"
    assets = {asset["name"]: asset for asset in release["assets"]}
    directory = ROOT / version
    directory.mkdir()
    for filename in (name, f"{name}.sha256"):
        asset = assets[filename]
        url = f"https://github.com/{REPO}/releases/download/release-v{version}/{filename}"
        require(asset["browser_download_url"] == url, "Unexpected asset URL")
        require(asset["digest"].startswith("sha256:"), "Missing GitHub digest")
        destination = directory / filename
        with urllib.request.urlopen(url, timeout=90) as response, destination.open("wb") as output:
            shutil.copyfileobj(response, output)
        require(destination.stat().st_size == asset["size"], "Asset size mismatch")
        require(digest(destination) == asset["digest"][7:], "GitHub asset SHA-256 mismatch")
    command(["shasum", "-a", "256", "-c", f"{name}.sha256"], cwd=directory)
    archive = directory / name
    prefix = name.removesuffix(".tar.gz")
    with tarfile.open(archive, "r:gz") as bundle:
        members = bundle.getmembers()
        for member in members:
            path = PurePosixPath(member.name)
            require(not path.is_absolute() and ".." not in path.parts and path.parts[0] == prefix,
                    "Unexpected archive path")
            require(member.isdir() or member.isfile(), "Archive contains a link or special file")
        require(sum(member.size for member in members) < 768 * 1024 * 1024, "Archive is too large")
    command(["tar", "-xzf", str(archive)], cwd=directory)
    extracted = directory / prefix
    command(["shasum", "-a", "256", "-c", "SHA256SUMS.txt"], cwd=extracted,
            log=f"manifest-{version}.log")
    require(command(["lipo", "-archs", str(extracted / "grok-zh")]).strip() == "arm64", "Not ARM64")
    require(command([str(extracted / "grok-zh"), "--version"]).startswith(f"grok-zh {version} ("),
            "Unexpected published binary version")
    return extracted


def install(old_package, name, auto_update=None):
    home = ROOT / name
    env = {key: value for key, value in os.environ.items()
           if not key.startswith(("GROK_", "XAI_")) and key not in ("GH_TOKEN", "GITHUB_TOKEN")}
    env.update(GROK_HOME=str(home), GROK_AUTH_PATH=str(home / "auth.json"),
               XAI_API_KEY="ci-updater-test-not-a-real-key", GROK_DISABLE_AUTOUPDATER="0",
               TERM="xterm-256color", COLUMNS="160", LINES="50")
    command([str(old_package / "Install-GrokZh.sh")], env=env, cwd=old_package, log=f"{name}-install.log")
    config = '[features]\nremote_fetch = false\n[cli]\nchannel = "stable"\n'
    if auto_update is not None:
        config += f"auto_update = {str(auto_update).lower()}\n"
    (home / "config.toml").write_text(config, encoding="utf-8")
    binary = home / "bin" / "grok-zh"
    require(command([str(binary), "--version"], env=env).startswith(f"grok-zh {OLD} ("),
            "Installer did not install the old release")
    env["PATH"] = f"{home / 'bin'}:{env['PATH']}"
    work = home / "workspace"
    work.mkdir()
    return home, binary, env, work


def verify_upgrade(home, binary, env, before, expected_hash):
    require(binary.is_symlink(), "Managed entry must remain a symlink")
    after = os.readlink(binary)
    require(after != before, "Managed entry did not switch")
    require(binary.resolve().parent == home / "grok-zh-downloads", "Unexpected managed target")
    require(digest(binary.resolve()) == expected_hash, "Installed binary differs from the published 1.0.13")
    require(command([str(binary), "--version"], env=env).startswith(f"grok-zh {NEW} ("), "New version did not launch")
    alias = home / "bin" / "agent-zh"
    require(alias.is_symlink() and alias.resolve() == binary.resolve(), "agent-zh alias did not follow update")
    require(command([str(alias), "--version"], env=env).startswith(f"grok-zh {NEW} ("), "Alias version mismatch")
    require(digest((binary.parent / before).resolve()) != expected_hash, "Old target was overwritten")
    for name in ("grok", "agent"):
        require(not (home / "bin" / name).exists(), "Unexpected compatibility alias")
    return {"before_link": before, "after_link": after, "installed_sha256": expected_hash,
            "next_launch_version": NEW, "old_target_preserved": True, "agent_alias_verified": True}


class Terminal:
    def __init__(self, args, env, cwd, name):
        import fcntl
        import pty
        import struct
        import termios
        self.pid, self.fd = pty.fork()
        if self.pid == 0:
            os.chdir(cwd)
            os.execvpe(args[0], args, env)
        fcntl.ioctl(self.fd, termios.TIOCSWINSZ, struct.pack("HHHH", 50, 160, 0, 0))
        self.transcript = (EVIDENCE / f"{name}-terminal.log").open("wb")
        self.output = b""
        self.status = None
        self.user_keys = []

    def poll(self):
        import select
        ready, _, _ = select.select([self.fd], [], [], 0.25)
        if ready:
            try:
                chunk = os.read(self.fd, 65536)
            except OSError:
                chunk = b""
            self.transcript.write(chunk)
            self.transcript.flush()
            self.output += chunk
            # Emulate terminal capability replies; these are not user actions.
            if b"\x1b[6n" in chunk:
                os.write(self.fd, b"\x1b[1;1R")
            if b"\x1b]11;?" in chunk:
                os.write(self.fd, b"\x1b]11;rgb:0000/0000/0000\x1b\\")
            if b"\x1b[c" in chunk:
                os.write(self.fd, b"\x1b[?1;2c")
        if self.status is None:
            done, status = os.waitpid(self.pid, os.WNOHANG)
            if done:
                self.status = os.waitstatus_to_exitcode(status)
        return self.status

    def key(self, key, label):
        self.user_keys.append(label)
        os.write(self.fd, key)

    def close(self):
        import signal
        if self.status is None:
            os.kill(self.pid, signal.SIGTERM)
            deadline = time.monotonic() + 10
            while self.poll() is None and time.monotonic() < deadline:
                pass
            if self.status is None:
                os.kill(self.pid, signal.SIGKILL)
                os.waitpid(self.pid, 0)
        self.transcript.close()
        os.close(self.fd)


def tui_case(old_package, expected_hash, automatic):
    name = "background-enabled" if automatic else "default-ctrl-u"
    home, binary, env, work = install(old_package, name, True if automatic else None)
    before = os.readlink(binary)
    assert_stable()
    terminal = Terminal([str(binary), "--no-leader", "--trust", "--no-alt-screen"], env, work, name)
    started = time.monotonic()
    prompt_at = None
    submitted = False
    try:
        deadline = started + 240
        while time.monotonic() < deadline:
            status = terminal.poll()
            if automatic and os.readlink(binary) != before:
                require(status is None, "Old process exited before background activation was observed")
                details = verify_upgrade(home, binary, env, before, expected_hash)
                details.update(user_keys_before_activation=terminal.user_keys.copy(), old_process_alive=True,
                               elapsed_seconds=round(time.monotonic() - started, 2), automatic_relaunch=False)
                require(not terminal.user_keys, "Automatic update required a user action")
                return {"case": name, "passed": True, **details}
            if not automatic:
                plain = re.sub(rb"\x1b\[[0-?]*[ -/]*[@-~]", b"", terminal.output).lower()
                if prompt_at is None and b"1.0.13" in plain and b"ctrl+u" in plain:
                    prompt_at = time.monotonic()
                if not submitted:
                    require(os.readlink(binary) == before, "Default-off mode installed without consent")
                if prompt_at and not submitted and time.monotonic() - prompt_at >= 20:
                    terminal.key(b"\x15", "Ctrl+U")
                    submitted = True
                if status is not None:
                    require(submitted and status == 0, f"TUI exited before successful Ctrl+U update: {status}")
                    details = verify_upgrade(home, binary, env, before, expected_hash)
                    return {"case": name, "passed": True, **details, "user_keys": terminal.user_keys,
                            "default_off_observed_seconds": 20, "automatic_relaunch": False}
            require(status is None, f"TUI exited unexpectedly: {status}")
        raise AssertionError(f"{name}: timed out waiting for update; inspect terminal evidence")
    finally:
        terminal.close()


def cli_case(old_package, expected_hash):
    home, binary, env, work = install(old_package, "explicit-cli")
    before = os.readlink(binary)
    assert_stable()
    status = command([str(binary), "update", "--check", "--json"], env=env, cwd=work,
                     log="explicit-cli-before.json")
    parsed = json.loads(status)
    require(parsed.get("currentVersion") == OLD and parsed.get("latestVersion") == NEW
            and parsed.get("updateAvailable") is True and not parsed.get("error"),
            "Update status did not discover the exact old-to-new transition")
    command([str(binary), "update"], env=env, cwd=work, log="explicit-cli-update.log")
    details = verify_upgrade(home, binary, env, before, expected_hash)
    after = os.readlink(binary)
    command([str(binary), "update"], env=env, cwd=work, log="explicit-cli-repeat.log")
    require(os.readlink(binary) == after, "Repeated up-to-date check replaced the binary again")
    return {"case": "explicit-cli", "passed": True, **details, "repeat_update_noop": True}


def main():
    require(platform.system() == "Darwin" and platform.machine() == "arm64", "Requires native macOS ARM64")
    check_runner_api_budget()
    assert_stable()
    old_package = package(OLD)
    new_package = package(NEW)
    expected_hash = digest(new_package / "grok-zh")
    for name, run in (("explicit-cli", lambda: cli_case(old_package, expected_hash)),
                      ("background-enabled", lambda: tui_case(old_package, expected_hash, True)),
                      ("default-ctrl-u", lambda: tui_case(old_package, expected_hash, False))):
        print(f"Starting {name}", flush=True)
        try:
            result = run()
        except Exception as error:
            traceback.print_exc()
            result = {"case": name, "passed": False, "error": str(error)}
        RESULTS.append(result)
        print(json.dumps(result), flush=True)
    require(all(result["passed"] for result in RESULTS), "At least one release update case failed")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        if not RESULTS:
            RESULTS.append({"case": "environment-prerequisites", "passed": False, "error": str(error)})
        raise
    finally:
        (EVIDENCE / "results.json").write_text(json.dumps(RESULTS, indent=2), encoding="utf-8")
        summary = ["## macOS published-release update test", f"Transition: {OLD} → {NEW}",
                   "Uses unmodified immutable release binaries and the real GitHub stable feed.", "",
                   "| Case | Result |", "| --- | --- |"]
        summary.extend(f"| {item['case']} | {'PASS' if item['passed'] else 'FAIL'} |" for item in RESULTS)
        summary += ["", "Background updates are opt-in. A successful background case verifies disk activation",
                    "while the old process stays alive; it does not claim automatic process restart.",
                    "No real xAI account is used. Gatekeeper and notarization are outside this test."]
        if os.environ.get("GITHUB_STEP_SUMMARY"):
            with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as output:
                output.write("\n".join(summary) + "\n")
