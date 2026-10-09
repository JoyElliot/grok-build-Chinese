"""Remove this workspace's own Cargo outputs from a target profile before caching it.

Workspace crates are rebuilt on every CI run (checkout refreshes source mtimes and
the version/commit are injected at build time), so caching their outputs only
spends cache quota.  Third-party outputs are left untouched: their fingerprints do
not reference workspace units, so later runs keep reusing them.

Workspace names come from ``cargo metadata --no-deps``; nothing is hard-coded.
Any metadata failure is fatal so an unexpectedly large cache is never saved silently.
"""

import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys


UNIT_HASH = re.compile(r"^(?P<name>.+)-(?P<hash>[0-9a-f]{16})$")
INCREMENTAL = re.compile(r"^(?P<name>.+)-(?P<hash>[0-9a-z]{6,})$")
LIB_PREFIXED = {".rlib", ".rmeta", ".so", ".dylib", ".a"}


class WorkspaceNames:
    def __init__(self, packages, crates, binaries):
        if not packages or not crates:
            raise ValueError("cargo metadata returned no workspace members")
        self.packages = frozenset(packages)
        self.crates = frozenset(crates)
        self.binaries = frozenset(binaries)

    @classmethod
    def from_metadata(cls, metadata):
        members = set(metadata.get("workspace_members") or [])
        packages, crates, binaries = set(), set(), set()
        for package in metadata.get("packages", []):
            if package.get("id") not in members:
                continue
            packages.add(package["name"])
            for target in package.get("targets", []):
                kinds = set(target.get("kind", []))
                if kinds == {"custom-build"}:
                    # build_script_build lives under build/<package>-<hash>.
                    continue
                crates.add(target["name"].replace("-", "_"))
                if kinds & {"bin", "example", "test", "bench"}:
                    binaries.add(target["name"])
        return cls(packages, crates, binaries)


def load_metadata(manifest_path, cargo):
    command = [cargo, "metadata", "--format-version", "1", "--no-deps", "--offline"]
    if manifest_path:
        command += ["--manifest-path", str(manifest_path)]
    result = subprocess.run(command, check=False, capture_output=True, text=True,
                            encoding="utf-8", errors="replace")
    if result.returncode != 0:
        raise RuntimeError("cargo metadata failed:\n" + result.stderr.strip())
    return json.loads(result.stdout)


def split_ext(name):
    """Return (stem, extension) where extension keeps the leading dot."""
    if name.endswith(".dSYM"):
        return name[:-5], ".dSYM"
    stem, ext = os.path.splitext(name)
    return stem, ext


def crate_from_unit_file(name, workspace):
    stem, ext = split_ext(name)
    match = UNIT_HASH.match(stem)
    if not match:
        return None
    crate = match.group("name")
    if ext in LIB_PREFIXED and crate.startswith("lib"):
        crate = crate[3:]
    return crate if crate in workspace.crates else None


def uplifted_owner(name, workspace):
    stem, ext = split_ext(name)
    # Cargo only uplifts requested root units; libfoo.d is the dep-info of libfoo.rlib.
    if (ext in LIB_PREFIXED or ext == ".d") and stem.startswith("lib") and stem[3:] in workspace.crates:
        return stem[3:]
    if stem in workspace.binaries or stem in workspace.crates:
        return stem
    return None


def size_of(path):
    if path.is_symlink() or path.is_file():
        try:
            return path.lstat().st_size
        except OSError:
            return 0
    total = 0
    for root, _dirs, files in os.walk(path):
        for file in files:
            try:
                total += os.lstat(os.path.join(root, file)).st_size
            except OSError:
                pass
    return total


def plan(profile_dir, workspace):
    """Yield (category, path) for every workspace-owned entry in a profile dir."""
    profile_dir = Path(profile_dir)
    for category in (".fingerprint", "build"):
        directory = profile_dir / category
        if directory.is_dir():
            for entry in directory.iterdir():
                match = UNIT_HASH.match(entry.name)
                if match and match.group("name") in workspace.packages:
                    yield category, entry
    for category in ("deps", "examples"):
        directory = profile_dir / category
        if directory.is_dir():
            for entry in directory.iterdir():
                if crate_from_unit_file(entry.name, workspace):
                    yield category, entry
    directory = profile_dir / "incremental"
    if directory.is_dir():
        for entry in directory.iterdir():
            match = INCREMENTAL.match(entry.name)
            if match and match.group("name") in workspace.crates:
                yield "incremental", entry
    if profile_dir.is_dir():
        for entry in profile_dir.iterdir():
            if entry.is_file() and uplifted_owner(entry.name, workspace):
                yield "uplifted", entry
            elif entry.is_dir() and entry.name.endswith(".dSYM") and uplifted_owner(entry.name, workspace):
                yield "uplifted", entry


def remove(path):
    if path.is_dir() and not path.is_symlink():
        shutil.rmtree(path)
    else:
        path.unlink()


def prune(profile_dirs, workspace, dry_run=False):
    report = []
    for profile_dir in profile_dirs:
        profile_dir = Path(profile_dir)
        row = {"profile": str(profile_dir), "exists": profile_dir.is_dir(),
               "before": 0, "removed": {}, "entries": 0}
        if row["exists"]:
            row["before"] = size_of(profile_dir)
            for category, entry in list(plan(profile_dir, workspace)):
                row["removed"][category] = row["removed"].get(category, 0) + size_of(entry)
                row["entries"] += 1
                if not dry_run:
                    remove(entry)
        row["removed_total"] = sum(row["removed"].values())
        row["after"] = row["before"] - row["removed_total"]
        report.append(row)
    return report


def gib(value):
    return f"{value / 1024 ** 3:.2f} GiB"


def render(report, workspace, dry_run):
    verb = "将删除" if dry_run else "已删除"
    lines = [f"本仓 workspace：{len(workspace.packages)} 个包，{len(workspace.crates)} 个 crate 名"]
    for row in report:
        if not row["exists"]:
            lines.append(f"- {row['profile']}：目录不存在，跳过")
            continue
        share = row["removed_total"] / row["before"] * 100 if row["before"] else 0.0
        parts = "，".join(f"{k} {gib(v)}" for k, v in sorted(row["removed"].items()))
        lines.append(
            f"- {row['profile']}：原 {gib(row['before'])}，{verb}本仓产物 {gib(row['removed_total'])}"
            f"（{share:.1f}%，{row['entries']} 项；{parts or '无'}），保留 {gib(row['after'])}"
        )
    return "\n".join(lines)


def main(argv=None):
    for stream in (sys.stdout, sys.stderr):
        # Windows runners default to a legacy code page; the report is Chinese.
        if hasattr(stream, "reconfigure"):
            stream.reconfigure(encoding="utf-8", errors="replace")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile-dir", action="append", default=[],
                        help="Cargo profile directory, e.g. target/<triple>/release-dist; repeatable")
    parser.add_argument("--profile-dir-list", default="",
                        help="Newline-separated profile directories (the composite action input)")
    parser.add_argument("--manifest-path", default=None)
    parser.add_argument("--metadata-json", default=None,
                        help="Use a saved `cargo metadata --no-deps` JSON instead of running cargo")
    parser.add_argument("--cargo", default=os.environ.get("CARGO", "cargo"))
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args(argv)

    try:
        if args.metadata_json:
            metadata = json.loads(Path(args.metadata_json).read_text(encoding="utf-8"))
        else:
            metadata = load_metadata(args.manifest_path, args.cargo)
        workspace = WorkspaceNames.from_metadata(metadata)
    except (OSError, RuntimeError, ValueError, KeyError) as error:
        print(f"::error::无法确定本仓 workspace crate 名单，拒绝保存未清理的缓存：{error}",
              file=sys.stderr)
        return 1

    profile_dirs = [p.strip() for p in args.profile_dir + args.profile_dir_list.splitlines() if p.strip()]
    if not profile_dirs:
        print("::error::未提供需要清理的 Cargo profile 目录", file=sys.stderr)
        return 1
    report = prune(profile_dirs, workspace, args.dry_run)
    text = render(report, workspace, args.dry_run)
    print(text)
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary and not args.dry_run:
        with open(summary, "a", encoding="utf-8") as handle:
            handle.write("### 缓存保存前清理本仓产物\n\n" + text + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
