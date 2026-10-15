#!/usr/bin/env python3
"""Plan/apply narrowly scoped preview artifact cleanup. No run/log/release writes.

The only mutating API is DELETE /actions/artifacts/{id}, behind --apply and
fresh provenance/completeness checks. State must be uploaded before --apply.
"""
from __future__ import annotations

import argparse
import copy
from datetime import datetime, timedelta, timezone
import io
import json
import os
from pathlib import Path
import re
import subprocess
import zipfile

CI_PATH = ".github/workflows/zh-dev-windows-preview.yml"
CLEANUP_PATH = ".github/workflows/preview-retention.yml"
BRANCHES = frozenset({"zh-dev", "codex/free-account-localization-macos-smoke"})
EVENTS = frozenset({"push", "schedule", "workflow_dispatch"})
STATE_PREFIX = "preview-retention-state-v2"
SCHEMA = 2
GRACE = timedelta(days=14)
MAX_AGE = timedelta(days=90)
SYMBOL_MAX_AGE = timedelta(days=30)
REQUIRED_JOBS = frozenset({
    "六目标构建汇总", "Windows x64 GNU 预览版", "Windows ARM64 MSVC 预览版",
    "Linux x64 GNU 预览版", "Linux ARM64 GNU 预览版", "macOS ARM64 预览版",
    "macOS Intel 预览版", "macOS Intel 原生产物验证",
})
TARGETS = frozenset({
    "windows-aarch64-msvc", "linux-x86_64-gnu", "linux-aarch64-gnu",
    "macos-aarch64", "macos-x86_64",
})
VERSION = r"(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)\.(?:0|[1-9][0-9]*)(?:-[0-9A-Za-z.-]+)?"
PACKAGE = re.compile(rf"grok-zh-({VERSION})-({'|'.join(sorted(TARGETS))})-([1-9][0-9]*)-([1-9][0-9]*)\Z")
WINDOWS = re.compile(r"grok-zh-windows(-symbols)?-([1-9][0-9]*)-([1-9][0-9]*)\Z")
STATE_NAME = re.compile(rf"{STATE_PREFIX}-([1-9][0-9]*)-([1-9][0-9]*)\Z")


def timestamp(value):
    dt = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if dt.tzinfo is None:
        raise ValueError("Timezone required")
    return dt.astimezone(timezone.utc)


def iso(value):
    return value.astimezone(timezone.utc).isoformat().replace("+00:00", "Z")


def positive(value):
    if type(value) is not int or value <= 0:
        raise ValueError("Expected a positive integer")
    return value


def family(artifact, run):
    """Full-name match plus run/attempt binding; never delete by prefix."""
    name = artifact.get("name", "")
    match = WINDOWS.fullmatch(name)
    if match:
        kind, run_id, attempt = match.groups()
        result, version = ("symbols" if kind else "windows-x86_64-gnu"), None
    else:
        match = PACKAGE.fullmatch(name)
        if not match:
            return None
        version, result, run_id, attempt = match.groups()
        if len(version) > 64:
            return None
    if int(run_id) != run["id"] or not 1 <= int(attempt) <= run["run_attempt"]:
        return None
    return result, version, int(attempt)


def trusted_run(run, repository, workflow_id, path=CI_PATH):
    return (
        run.get("workflow_id") == workflow_id and run.get("path") == path
        and run.get("head_repository", {}).get("id") == repository["id"]
        and run.get("repository", {}).get("id") == repository["id"]
        and run.get("head_branch") in BRANCHES
        and run.get("event") in EVENTS and not run.get("pull_requests")
        and isinstance(run.get("head_sha"), str)
        and re.fullmatch(r"[0-9a-f]{40}", run["head_sha"]) is not None
        and all(type(run.get(k)) is int and run[k] > 0 for k in ("id", "run_number", "run_attempt"))
    )


def trusted_state_run(run, repository, cleanup_id):
    return (
        run.get("workflow_id") == cleanup_id and run.get("path") == CLEANUP_PATH
        and run.get("repository", {}).get("id") == repository["id"]
        and run.get("head_repository", {}).get("id") == repository["id"]
        and run.get("head_branch") == repository["default_branch"]
        and run.get("event") in {"schedule", "workflow_run", "workflow_dispatch"}
        and not run.get("pull_requests")
        and all(type(run.get(k)) is int and run[k] > 0 for k in ("id", "run_attempt"))
    )


def live_artifact(artifact, run, repository, now):
    origin = artifact.get("workflow_run", {})
    return (
        type(artifact.get("id")) is int and artifact["id"] > 0
        and not artifact.get("expired", True) and artifact.get("size_in_bytes", 0) > 0
        and timestamp(artifact["expires_at"]) > now
        and timestamp(artifact["created_at"]) <= now
        and origin.get("id") == run["id"]
        and origin.get("repository_id") == repository["id"]
        and origin.get("head_repository_id") == repository["id"]
        and origin.get("head_branch") == run["head_branch"]
        and origin.get("head_sha") == run["head_sha"]
    )


def complete_run(run, artifacts, jobs, repository, workflow_id, now, proof=None):
    if not trusted_run(run, repository, workflow_id) or (
        run.get("status") != "completed" or run.get("conclusion") != "success"
    ):
        return None
    found, versions = {}, set()
    for artifact in artifacts:
        parsed = family(artifact, run)
        if not parsed or parsed[2] != run["run_attempt"]:
            continue
        if not live_artifact(artifact, run, repository, now):
            continue
        if parsed[0] in found:
            return None  # Ambiguous versions/duplicate platform outputs fail closed.
        found[parsed[0]] = artifact
        if parsed[1]:
            versions.add(parsed[1])
    packages = TARGETS | {"windows-x86_64-gnu"}
    if not packages <= set(found) or len(versions) != 1:
        return None
    # A symbol's shorter native lifetime must not invalidate an otherwise live
    # 90-day preview. Only a trusted, persisted seven-artifact observation can
    # substitute for symbols, and only after their recorded native expiry.
    if "symbols" not in found:
        if not proof or timestamp(proof["artifacts"]["symbols"]["expires_at"]) > now:
            return None
        for kind, artifact in found.items():
            if proof["artifacts"].get(kind) != artifact_identity(artifact):
                return None
    by_name = {}
    for job in jobs:
        if job.get("run_id") != run["id"] or job.get("run_attempt") != run["run_attempt"]:
            return None
        by_name.setdefault(job.get("name"), []).append(job)
    if any(len(by_name.get(name, [])) != 1 or by_name[name][0].get("conclusion") != "success"
           or by_name[name][0].get("status") != "completed" for name in REQUIRED_JOBS):
        return None
    if any(job.get("status") != "completed" or not job.get("completed_at") for job in jobs):
        return None
    completed = max(timestamp(job["completed_at"]) for job in jobs)
    if completed > now or any(timestamp(a["created_at"]) > completed for a in found.values()):
        return None
    result = {"run_id": run["id"], "run_number": run["run_number"],
              "run_attempt": run["run_attempt"], "head_sha": run["head_sha"],
              "branch": run["head_branch"], "completed_at": iso(completed)}
    if "symbols" not in found and any(proof.get(k) != value for k, value in result.items()):
        return None
    return result


def proof_key(run):
    return f"{run['id']}-{run['run_attempt']}"


def artifact_identity(artifact):
    return {"artifact_id": artifact["id"], **{k: artifact[k] for k in
            ("name", "created_at", "expires_at")}}


def completeness_proof(complete, run, artifacts, repository, now):
    # Called only after a fresh seven-artifact verification, never to renew an
    # expired symbol or synthesize evidence from an older proof.
    return {**complete, "verified_at": iso(now), "artifacts": {
        parsed[0]: artifact_identity(artifact) for artifact in artifacts
        if (parsed := family(artifact, run)) and parsed[2] == run["run_attempt"]
        and live_artifact(artifact, run, repository, now)
    }}


def validate_proof(key, proof):
    for field in ("run_id", "run_number", "run_attempt"):
        positive(proof[field])
    if key != f"{proof['run_id']}-{proof['run_attempt']}" or proof["branch"] not in BRANCHES:
        raise ValueError("Invalid completeness proof identity")
    if not re.fullmatch(r"[0-9a-f]{40}", proof["head_sha"]):
        raise ValueError("Invalid completeness proof SHA")
    if set(proof["artifacts"]) != TARGETS | {"windows-x86_64-gnu", "symbols"}:
        raise ValueError("Incomplete proof")
    completed, verified = timestamp(proof["completed_at"]), timestamp(proof["verified_at"])
    fake_run = {"id": proof["run_id"], "run_attempt": proof["run_attempt"]}
    identities, versions = set(), set()
    for kind, artifact in proof["artifacts"].items():
        ident = positive(artifact["artifact_id"])
        parsed = family(artifact, fake_run)
        if not parsed or parsed[0] != kind or parsed[2] != proof["run_attempt"] or ident in identities:
            raise ValueError("Invalid proof artifact identity")
        identities.add(ident)
        if parsed[1]:
            versions.add(parsed[1])
        created, expiry = timestamp(artifact["created_at"]), timestamp(artifact["expires_at"])
        if not created <= completed <= verified < expiry or expiry > created + MAX_AGE + timedelta(hours=1):
            raise ValueError("Invalid completeness proof lifetime")
    if len(versions) != 1:
        raise ValueError("Mixed proof versions")


def empty_state(repository, workflow_id):
    return {"schema": SCHEMA, "repository_id": repository["id"],
            "workflow_id": workflow_id, "entries": {}, "proofs": {}}


def validate_state(state, repository, workflow_id):
    if (state.get("schema") != SCHEMA or state.get("repository_id") != repository["id"]
            or state.get("workflow_id") != workflow_id or not isinstance(state.get("entries"), dict)
            or not isinstance(state.get("proofs"), dict)):
        raise ValueError("Unrecognized retention state; refusing cleanup")
    for key, proof in state["proofs"].items():
        validate_proof(key, proof)
    for key, entry in state["entries"].items():
        if key != str(positive(entry["artifact_id"])) or entry["branch"] not in BRANCHES:
            raise ValueError("Invalid retention entry identity")
        for field in ("run_id", "run_number", "attempt", "superseded_by"):
            positive(entry[field])
        if not re.fullmatch(r"[0-9a-f]{40}", entry["head_sha"]):
            raise ValueError("Invalid entry SHA")
        fake_run = {"id": entry["run_id"], "run_attempt": entry["attempt"]}
        parsed = family({"name": entry["name"]}, fake_run)
        if not parsed or parsed[2] != entry["attempt"]:
            raise ValueError("Invalid entry name")
        first, expiry, created = (timestamp(entry[k]) for k in
                                  ("first_superseded_at", "original_expires_at", "created_at"))
        lifetime = SYMBOL_MAX_AGE if parsed[0] == "symbols" else MAX_AGE
        if expiry <= created or expiry > created + lifetime + timedelta(hours=1):
            raise ValueError("Invalid original artifact expiry")
        if timestamp(entry["delete_after"]) != min(first + GRACE, expiry):
            raise ValueError("Invalid deletion deadline")
    return state


def plan(repository, workflow_id, runs, artifacts_by_run, jobs_by_run, state, now, is_descendant=None):
    validate_state(state, repository, workflow_id)
    # Missing ancestry evidence never authorizes shortening an artifact lifetime.
    is_descendant = is_descendant or (lambda base, head: False)
    state = copy.deepcopy(state)
    state["entries"] = {key: value for key, value in state["entries"].items()
                        if timestamp(value["original_expires_at"]) > now}
    state["proofs"] = {key: proof for key, proof in state["proofs"].items()
                       if max(timestamp(a["expires_at"]) for kind, a in proof["artifacts"].items()
                              if kind != "symbols") > now}
    if any(timestamp(p["verified_at"]) > now for p in state["proofs"].values()):
        raise ValueError("Future completeness proof")
    candidates = []
    for run in runs:
        artifacts = artifacts_by_run.get(run["id"], [])
        jobs = jobs_by_run.get(run["id"], [])
        complete = complete_run(run, artifacts, jobs, repository, workflow_id, now)
        if complete:
            state["proofs"].setdefault(proof_key(run), completeness_proof(complete, run, artifacts, repository, now))
        else:
            complete = complete_run(run, artifacts, jobs, repository, workflow_id, now,
                                    state["proofs"].get(proof_key(run)))
        if complete:
            candidates.append(complete)
    deletions = []
    for run in runs:
        # Never interfere with a queued/active rerun or any pull request.
        if not trusted_run(run, repository, workflow_id) or run.get("status") != "completed":
            continue
        newer = [c for c in candidates if c["branch"] == run["head_branch"]
                 and c["run_number"] > run["run_number"]
                 and c["head_sha"] != run["head_sha"]]
        if not newer:
            continue  # Explicit last-good protection, even when a stale ledger says otherwise.
        successor = next((candidate for candidate in sorted(newer, key=lambda c: timestamp(c["completed_at"]))
                          if is_descendant(run["head_sha"], candidate["head_sha"])), None)
        if successor is None:
            continue  # Rewinds/diverged branches/old-commit dispatches cannot supersede.

        for artifact in artifacts_by_run.get(run["id"], []):
            parsed = family(artifact, run)
            if not parsed or not live_artifact(artifact, run, repository, now):
                continue
            # Do not retrofit the policy onto pre-deployment seven/fourteen-day artifacts.
            created = timestamp(artifact["created_at"])
            expiry = timestamp(artifact["expires_at"])
            lifetime = SYMBOL_MAX_AGE if parsed[0] == "symbols" else MAX_AGE
            if not lifetime - timedelta(days=1) <= expiry - created <= lifetime + timedelta(hours=1):
                continue
            key = str(artifact["id"])
            entry = {"artifact_id": artifact["id"], "name": artifact["name"],
                     "run_id": run["id"], "run_number": run["run_number"],
                     "attempt": parsed[2], "head_sha": run["head_sha"],
                     "branch": run["head_branch"], "created_at": artifact["created_at"],
                     "original_expires_at": artifact["expires_at"],
                     "first_superseded_at": successor["completed_at"],
                     "superseded_by": successor["run_id"]}
            old = state["entries"].get(key)
            if old:
                for field in ("name", "run_id", "run_number", "attempt", "head_sha", "branch", "created_at"):
                    if old[field] != entry[field]:
                        raise ValueError("Artifact identity changed; refusing cleanup")
                entry["original_expires_at"] = iso(min(expiry, timestamp(old["original_expires_at"])))
                if timestamp(old["first_superseded_at"]) <= timestamp(entry["first_superseded_at"]):
                    entry["first_superseded_at"] = old["first_superseded_at"]
                    entry["superseded_by"] = old["superseded_by"]
            entry["delete_after"] = iso(min(timestamp(entry["first_superseded_at"]) + GRACE,
                                            timestamp(entry["original_expires_at"])))
            state["entries"][key] = entry
            if timestamp(entry["delete_after"]) <= now:
                deletions.append(entry)
    state["planned_at"] = iso(now)
    validate_state(state, repository, workflow_id)
    return state, sorted(deletions, key=lambda e: e["artifact_id"])


class GitHub:
    def __init__(self, repository):
        if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
            raise ValueError("Invalid repository")
        self.repository = repository
        self.base = f"repos/{repository}"
        self._ancestry = {}

    def request(self, path, method="GET", binary=False):
        if not path.startswith(self.base + "/") and path != self.base:
            raise ValueError("API path escaped repository")
        if method != "GET" and not (method == "DELETE" and re.fullmatch(
                re.escape(self.base) + r"/actions/artifacts/[1-9][0-9]*", path)):
            raise ValueError("Only artifact deletion is supported")
        result = subprocess.run(["gh", "api", "--method", method, "-H",
                                 "X-GitHub-Api-Version: 2022-11-28", path],
                                check=True, stdout=subprocess.PIPE)
        if binary:
            return result.stdout
        return json.loads(result.stdout) if result.stdout else None

    def pages(self, path, key):
        results, expected = {}, None
        for page in range(1, 1001):
            separator = "&" if "?" in path else "?"
            payload = self.request(f"{path}{separator}per_page=100&page={page}")
            batch, total = payload[key], payload["total_count"]
            if not isinstance(batch, list) or type(total) is not int or total < 0:
                raise ValueError("Invalid paginated response")
            if expected is None:
                expected = total
            if total != expected:
                raise RuntimeError("Inventory changed during pagination; retry on next maintenance run")
            for item in batch:
                ident = positive(item["id"])
                if ident in results:
                    raise RuntimeError("Duplicate ID during pagination; refusing incomplete inventory")
                results[ident] = item
            if len(batch) < 100:
                if len(results) != expected:
                    raise RuntimeError("Truncated inventory; refusing cleanup")
                return list(results.values())
        raise RuntimeError("Pagination limit reached; refusing incomplete inventory")

    def is_descendant(self, base, head):
        if any(not isinstance(sha, str) or not re.fullmatch(r"[0-9a-f]{40}", sha)
               for sha in (base, head)):
            raise ValueError("Invalid comparison SHA")
        if base == head:
            return False
        pair = (base, head)
        if pair not in self._ancestry:
            comparison = self.request(f"{self.base}/compare/{base}...{head}?per_page=1")
            if (comparison.get("base_commit", {}).get("sha") != base
                    or comparison.get("status") not in {"ahead", "behind", "identical", "diverged"}):
                raise ValueError("Invalid commit ancestry response")
            forward = comparison["status"] == "ahead"
            if forward and (comparison.get("merge_base_commit", {}).get("sha") != base
                            or type(comparison.get("ahead_by")) is not int or comparison["ahead_by"] <= 0
                            or type(comparison.get("behind_by")) is not int or comparison["behind_by"] != 0):
                raise ValueError("Incomplete forward ancestry evidence")
            self._ancestry[pair] = forward
        return self._ancestry[pair]

    def workflow(self, path):
        result = self.request(f"{self.base}/actions/workflows/{Path(path).name}")
        if result["path"] != path:
            raise ValueError("Unexpected workflow path")
        return positive(result["id"])

    def run(self, run_id):
        return self.request(f"{self.base}/actions/runs/{positive(run_id)}")

    def artifacts(self, run_id):
        return self.pages(f"{self.base}/actions/runs/{positive(run_id)}/artifacts", "artifacts")

    def jobs(self, run):
        return self.pages(f"{self.base}/actions/runs/{positive(run['id'])}/attempts/"
                          f"{positive(run['run_attempt'])}/jobs", "jobs")

    def inventory(self, repository, workflow_id, now):
        # Start from live artifacts, not all historical runs. Old reruns remain
        # discoverable without spending the token budget on artifact-less history.
        all_artifacts = self.pages(f"{self.base}/actions/artifacts", "artifacts")
        grouped = {}
        for artifact in all_artifacts:
            origin = artifact.get("workflow_run", {})
            name = artifact.get("name", "")
            if (artifact.get("expired", True) or not (WINDOWS.fullmatch(name) or PACKAGE.fullmatch(name))
                    or origin.get("repository_id") != repository["id"]
                    or origin.get("head_repository_id") != repository["id"]
                    or origin.get("head_branch") not in BRANCHES):
                continue
            grouped.setdefault(positive(origin["id"]), []).append(artifact)
        runs, artifacts, jobs = [], {}, {}
        for run_id, batch in grouped.items():
            run = self.run(run_id)
            if not trusted_run(run, repository, workflow_id):
                continue
            runs.append(run)
            artifacts[run_id] = batch
            if run.get("status") == "completed" and run.get("conclusion") == "success" and any(
                    family(a, run) and live_artifact(a, run, repository, now) for a in batch):
                jobs[run_id] = self.jobs(run)
        return runs, artifacts, jobs

    def state_artifact(self, artifact_id, repository, cleanup_id):
        artifact = self.request(f"{self.base}/actions/artifacts/{positive(artifact_id)}")
        match = STATE_NAME.fullmatch(artifact.get("name", ""))
        if not match or artifact.get("expired", True):
            raise ValueError("Invalid state artifact")
        run_id, attempt = map(int, match.groups())
        run = self.run(run_id)
        origin = artifact.get("workflow_run", {})
        if (not trusted_state_run(run, repository, cleanup_id) or attempt > run["run_attempt"]
                or origin.get("id") != run_id or origin.get("repository_id") != repository["id"]
                or origin.get("head_repository_id") != repository["id"]
                or origin.get("head_sha") != run["head_sha"]
                or origin.get("head_branch") != repository["default_branch"]):
            raise ValueError("Untrusted state artifact provenance")
        data = self.request(f"{self.base}/actions/artifacts/{artifact_id}/zip", binary=True)
        if len(data) > 4_000_000:
            raise ValueError("State archive too large")
        with zipfile.ZipFile(io.BytesIO(data)) as archive:
            if archive.namelist() != ["state.json"] or archive.getinfo("state.json").file_size > 4_000_000:
                raise ValueError("Unexpected state archive contents")
            return json.loads(archive.read("state.json"))

    def load_state(self, repository, workflow_id, cleanup_id):
        artifacts = self.pages(f"{self.base}/actions/artifacts", "artifacts")
        matches = [a for a in artifacts if STATE_NAME.fullmatch(a.get("name", ""))
                   and not a.get("expired", True)]
        # A failed cleanup may already have persisted state. Do not filter by conclusion.
        for artifact in sorted(matches, key=lambda a: a["id"], reverse=True):
            origin = artifact.get("workflow_run", {})
            run = self.run(positive(origin["id"]))
            if not trusted_state_run(run, repository, cleanup_id):
                continue  # Ignore PR/fork/non-default lookalikes, even from the same workflow.
            return validate_state(self.state_artifact(artifact["id"], repository, cleanup_id),
                                  repository, workflow_id)
        return empty_state(repository, workflow_id)


def apply_plan(api, repository, workflow_id, state, now, clock=None):
    """Re-inventory after state upload. Race or evidence failure retains artifacts."""
    clock = clock or (lambda: now)
    runs, artifacts, jobs = api.inventory(repository, workflow_id, now)
    _, eligible = plan(repository, workflow_id, runs, artifacts, jobs, state, now, api.is_descendant)
    run_map = {r["id"]: r for r in runs}
    deleted = []
    for entry in eligible:
        # Only already-persisted decisions can be applied, never newly discovered ones.
        if entry != state["entries"].get(str(entry["artifact_id"])):
            continue
        checked_at = clock()
        current = api.run(entry["run_id"])
        old = run_map[entry["run_id"]]
        if any(current.get(k) != old.get(k) for k in
               ("run_attempt", "status", "conclusion", "head_sha", "run_number")):
            raise RuntimeError("Run changed during cleanup; stopped before deletion")
        if not trusted_run(current, repository, workflow_id) or current["status"] != "completed":
            raise RuntimeError("Target run no longer eligible")
        # Revalidate a replacement immediately before DELETE; a rerun or missing
        # artifact must not permit stale state to delete the remaining last-good.
        newer = sorted((r for r in runs if r["head_branch"] == entry["branch"]
                        and r["run_number"] > entry["run_number"]
                        and r["head_sha"] != entry["head_sha"]),
                       key=lambda r: r["run_number"], reverse=True)
        replacement = None
        for candidate in newer:
            fresh = api.run(candidate["id"])
            if (fresh.get("status") != "completed" or fresh.get("conclusion") != "success"
                    or fresh.get("head_branch") != entry["branch"]
                    or fresh.get("run_number", 0) <= entry["run_number"]
                    or not api.is_descendant(entry["head_sha"], fresh.get("head_sha"))):
                continue
            replacement_artifacts, replacement_jobs = api.artifacts(fresh["id"]), api.jobs(fresh)
            checked_at = clock()
            if complete_run(fresh, replacement_artifacts, replacement_jobs, repository, workflow_id, checked_at,
                            state["proofs"].get(proof_key(fresh))):
                replacement = fresh
                break
        if not replacement:
            raise RuntimeError("No complete newer replacement remains; stopped cleanup")
        artifact = api.request(f"{api.base}/actions/artifacts/{entry['artifact_id']}")
        checked_at = clock()
        if (not live_artifact(artifact, current, repository, checked_at)
                or artifact["name"] != entry["name"]
                or artifact["created_at"] != entry["created_at"]
                or not family(artifact, current)):
            raise RuntimeError("Artifact changed during cleanup; stopped before deletion")
        if not complete_run(replacement, replacement_artifacts, replacement_jobs,
                            repository, workflow_id, checked_at, state["proofs"].get(proof_key(replacement))):
            raise RuntimeError("Replacement expired during cleanup; stopped before deletion")
        api.request(f"{api.base}/actions/artifacts/{entry['artifact_id']}", method="DELETE")
        deleted.append(entry["artifact_id"])
    return deleted


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", default=os.environ.get("GITHUB_REPOSITORY"))
    parser.add_argument("--output", type=Path, default=Path("retention-plan"))
    parser.add_argument("--apply", action="store_true", help="Delete only after verifying uploaded state")
    parser.add_argument("--state-artifact-id", type=int)
    args = parser.parse_args()
    api = GitHub(args.repository or "")
    repository = api.request(api.base)
    if repository["default_branch"] != "zh-dev":
        raise ValueError("Policy requires zh-dev as the trusted default branch")
    workflow_id, cleanup_id = api.workflow(CI_PATH), api.workflow(CLEANUP_PATH)
    now = datetime.now(timezone.utc)
    if args.apply:
        if not args.state_artifact_id:
            parser.error("--apply requires the successfully uploaded --state-artifact-id")
        state = validate_state(api.state_artifact(args.state_artifact_id, repository, cleanup_id),
                               repository, workflow_id)
        if now - timestamp(state["planned_at"]) > timedelta(hours=1) or timestamp(state["planned_at"]) > now:
            raise ValueError("Apply requires a fresh persisted plan")
        deleted = apply_plan(api, repository, workflow_id, state, now,
                             clock=lambda: datetime.now(timezone.utc))
        print(json.dumps({"deleted_artifact_ids": deleted}, indent=2))
        return
    state = api.load_state(repository, workflow_id, cleanup_id)
    runs, artifacts, jobs = api.inventory(repository, workflow_id, now)
    state, deletions = plan(repository, workflow_id, runs, artifacts, jobs, state, now, api.is_descendant)
    args.output.mkdir(parents=True, exist_ok=True)
    (args.output / "state.json").write_text(json.dumps(state, indent=2) + "\n", encoding="utf-8")
    report = {"mode": "plan-only", "planned_at": iso(now), "delete_candidates": deletions,
              "tracked_artifacts": len(state["entries"]), "verified_previews": len(state["proofs"])}
    (args.output / "report.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(report, indent=2))
    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    if summary:
        with open(summary, "a", encoding="utf-8") as file:
            file.write(f"## 预览制品保留计划\n\n待清理：{len(deletions)} 个；记录替换时间："
                       f"{len(state['entries'])} 个。仅六平台预览包及对应 Windows 符号。\n")


if __name__ == "__main__":
    main()
