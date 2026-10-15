"""Offline safety fixtures. Never contacts GitHub or deletes live artifacts."""
import copy
from datetime import datetime, timedelta, timezone
import importlib.util
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock
import zipfile

sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location("retention", ROOT / ".github/scripts/preview-retention.py")
r = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(r)
BASE = datetime(2026, 1, 1, tzinfo=timezone.utc)
REPO = {"id": 42, "default_branch": "zh-dev"}
WORKFLOW = 7
CLEANUP = 8


def run(number, day=0, **changes):
    value = {"id": number + 100, "run_number": number, "run_attempt": 1,
             "workflow_id": WORKFLOW, "path": r.CI_PATH, "repository": {"id": 42},
             "head_repository": {"id": 42}, "head_branch": "zh-dev", "head_sha": f"{number:040x}",
             "event": "push", "pull_requests": [], "status": "completed", "conclusion": "success",
             "created_at": r.iso(BASE + timedelta(days=day))}
    value.update(changes)
    return value


def artifacts(item, day=0, attempt=None, version="1.0.99-zh.ci.1"):
    attempt = attempt or item["run_attempt"]
    suffix = f"{item['id']}-{attempt}"
    names = [f"grok-zh-windows-{suffix}", f"grok-zh-windows-symbols-{suffix}"]
    names += [f"grok-zh-{version}-{target}-{suffix}" for target in sorted(r.TARGETS)]
    created = BASE + timedelta(days=day, hours=1)
    return [{"id": item["id"] * 10000 + attempt * 100 + index,
             "name": name, "created_at": r.iso(created), "expires_at": r.iso(created + (r.SYMBOL_MAX_AGE if "-symbols-" in name else r.MAX_AGE)),
             "expired": False, "size_in_bytes": 500, "workflow_run": {
                 "id": item["id"], "repository_id": 42, "head_repository_id": 42,
                 "head_branch": item["head_branch"], "head_sha": item["head_sha"],
             }} for index, name in enumerate(names)]


def jobs(item, day=0):
    return [{"id": item["id"] * 100 + index, "run_id": item["id"],
             "run_attempt": item["run_attempt"], "name": name, "status": "completed",
             "conclusion": "success", "started_at": r.iso(BASE + timedelta(days=day)),
             "completed_at": r.iso(BASE + timedelta(days=day, hours=2))}
            for index, name in enumerate(sorted(r.REQUIRED_JOBS))]


def ancestry(base, head):
    """Synthetic commit graph: fixture SHAs form one forward-only chain."""
    return int(base, 16) < int(head, 16)


class RetentionTests(unittest.TestCase):
    def setUp(self):
        self.old, self.new = run(1), run(2, day=10)
        self.runs = [self.old, self.new]
        self.artifacts = {self.old["id"]: artifacts(self.old), self.new["id"]: artifacts(self.new, 10)}
        self.jobs = {self.old["id"]: jobs(self.old), self.new["id"]: jobs(self.new, 10)}
        self.state = r.empty_state(REPO, WORKFLOW)

    def plan(self, day=25, state=None):
        return r.plan(REPO, WORKFLOW, self.runs, self.artifacts, self.jobs,
                      state or self.state, BASE + timedelta(days=day), ancestry)

    def test_fourteen_days_start_at_success_not_old_upload(self):
        state, deleted = self.plan(16)
        self.assertEqual(deleted, [])
        self.assertEqual(len(state["entries"]), 7)
        self.assertEqual({e["delete_after"] for e in state["entries"].values()},
                         {r.iso(BASE + timedelta(days=24, hours=2))})
        _, deleted = self.plan(25, state)
        self.assertEqual(len(deleted), 7)
        self.assertTrue(all(e["run_id"] == self.old["id"] for e in deleted))

    def test_boundary_is_exact(self):
        state, _ = self.plan(16)
        for delta, expected in [(timedelta(seconds=-1), 0), (timedelta(), 7)]:
            _, deleted = r.plan(REPO, WORKFLOW, self.runs, self.artifacts, self.jobs, state,
                                BASE + timedelta(days=24, hours=2) + delta, ancestry)
            self.assertEqual(len(deleted), expected)

    def test_latest_last_good_is_never_shortened_without_newer_complete(self):
        self.runs = [self.old]
        state, deleted = self.plan(89)
        self.assertEqual(state["entries"], {})
        self.assertEqual(deleted, [])

    def test_failed_cancelled_active_or_partial_success_never_supersedes(self):
        for status, conclusion in [("completed", "failure"), ("completed", "cancelled"),
                                   ("in_progress", None), ("queued", None)]:
            with self.subTest(status=status, conclusion=conclusion):
                self.new.update(status=status, conclusion=conclusion)
                self.assertEqual(self.plan()[1], [])
        self.new.update(status="completed", conclusion="success")
        self.artifacts[self.new["id"]].pop()
        self.assertEqual(self.plan()[1], [])

    def test_missing_matching_symbols_does_not_replace_old_complete_set(self):
        self.artifacts[self.new["id"]] = [a for a in self.artifacts[self.new["id"]]
                                           if "-symbols-" not in a["name"]]
        self.assertEqual(self.plan()[1], [])

    def test_required_native_and_aggregate_jobs_must_succeed(self):
        for name in r.REQUIRED_JOBS:
            with self.subTest(job=name):
                self.jobs[self.new["id"]] = jobs(self.new, 10)
                next(j for j in self.jobs[self.new["id"]] if j["name"] == name)["conclusion"] = "skipped"
                self.assertEqual(self.plan()[1], [])

    def test_skipped_optional_smoke_is_allowed(self):
        self.jobs[self.new["id"]].append({**jobs(self.new, 10)[0], "id": 9999,
                                         "name": "optional smoke", "conclusion": "skipped"})
        self.assertEqual(len(self.plan()[1]), 7)

    def test_wrong_workflow_repo_branch_pr_and_fork_never_supersede(self):
        for changes in [{"workflow_id": 999}, {"path": ".github/workflows/release.yml"},
                        {"head_repository": {"id": 99}}, {"repository": {"id": 99}},
                        {"head_branch": "sync/upstream"}, {"event": "pull_request"},
                        {"pull_requests": [{"number": 1}]},
                        {"head_branch": "codex/free-account-localization-macos-smoke"}]:
            with self.subTest(changes=changes):
                original = copy.deepcopy(self.new)
                self.new.update(changes)
                self.assertEqual(self.plan()[1], [])
                self.new.clear()
                self.new.update(original)

    def test_wrong_provenance_and_name_spoof_artifacts_are_untouched(self):
        extra = copy.deepcopy(self.artifacts[self.old["id"]][0])
        extra["id"] = 999
        for name in ["grok-zh-windows-101-1-extra", "grok-zh-windows-999-1", "grok-zh-windows-101-99",
                     "grok-msvc-cross-input-101-1", "migration-experiment-grok-zh-1.0.0-windows-x86_64-msvc-101-1",
                     "grok-zh-windows-gnu-input-101-1", "grok-zh-windows-symbols-101-1.zip"]:
            with self.subTest(name=name):
                extra["name"] = name
                self.artifacts[self.old["id"]].append(copy.deepcopy(extra))
        spoof = copy.deepcopy(self.artifacts[self.old["id"]][0])
        spoof["id"] = 1000
        spoof["workflow_run"]["head_repository_id"] = 999
        self.artifacts[self.old["id"]].append(spoof)
        self.assertEqual(len(self.plan()[1]), 7)

    def test_expired_missing_empty_duplicate_or_mixed_versions_do_not_replace(self):
        baseline = copy.deepcopy(self.artifacts[self.new["id"]])
        mutations = [lambda a: a[0].update(expired=True), lambda a: a[0].update(size_in_bytes=0),
                     lambda a: a.append(copy.deepcopy(a[0])),
                     lambda a: a[-1].update(name=a[-1]["name"].replace("1.0.99", "1.0.98"))]
        for mutate in mutations:
            with self.subTest(mutation=mutate):
                current = copy.deepcopy(baseline)
                mutate(current)
                self.artifacts[self.new["id"]] = current
                self.assertEqual(self.plan()[1], [])

    def test_later_old_rerun_never_replaces_newer_run(self):
        self.old["run_attempt"] = 2
        self.artifacts[self.old["id"]] += artifacts(self.old, 20)
        self.jobs[self.old["id"]] = jobs(self.old, 20)
        state, deleted = self.plan(25)
        self.assertTrue(all(e["run_id"] == self.old["id"] for e in deleted))
        self.assertEqual(len(deleted), 14)
        self.assertTrue(all(e["first_superseded_at"] == r.iso(BASE + timedelta(days=10, hours=2))
                            for e in state["entries"].values()))

    def test_partial_rerun_and_attempt_mismatch_fail_closed(self):
        self.new["run_attempt"] = 2
        self.jobs[self.new["id"]] = jobs(self.new, 11)
        self.assertEqual(self.plan()[1], [])  # Existing outputs are attempt 1.
        self.artifacts[self.new["id"]] = artifacts(self.new, 11)
        self.jobs[self.new["id"]].pop()
        self.assertEqual(self.plan()[1], [])
        self.jobs[self.new["id"]] = jobs(self.new, 11)
        self.jobs[self.new["id"]][0]["run_attempt"] = 1
        self.assertEqual(self.plan()[1], [])

    def test_first_supersession_survives_reruns_and_subsequent_updates(self):
        state, _ = self.plan(16)
        self.new["run_attempt"] = 2
        self.artifacts[self.new["id"]] = artifacts(self.new, 15)
        self.jobs[self.new["id"]] = jobs(self.new, 15)
        later = run(3, 16)
        self.runs.append(later)
        self.artifacts[later["id"]], self.jobs[later["id"]] = artifacts(later, 16), jobs(later, 16)
        state, deleted = self.plan(25, state)
        self.assertEqual(len(deleted), 7)
        self.assertEqual({e["first_superseded_at"] for e in deleted},
                         {r.iso(BASE + timedelta(days=10, hours=2))})

    def test_out_of_order_completion_uses_first_valid_success(self):
        self.jobs[self.new["id"]] = jobs(self.new, 14)
        later = run(3, 11)
        self.runs.append(later)
        self.artifacts[later["id"]], self.jobs[later["id"]] = artifacts(later, 11), jobs(later, 11)
        state, _ = self.plan(16)
        old_entries = [e for e in state["entries"].values() if e["run_id"] == self.old["id"]]
        self.assertEqual({e["first_superseded_at"] for e in old_entries},
                         {r.iso(BASE + timedelta(days=11, hours=2))})

    def test_original_expiry_caps_grace_and_no_extension(self):
        self.artifacts[self.new["id"]] = artifacts(self.new, 88)
        self.jobs[self.new["id"]] = jobs(self.new, 88)
        state, deleted = self.plan(89)
        self.assertEqual(deleted, [])
        self.assertTrue(all(e["delete_after"] == e["original_expires_at"] for e in state["entries"].values()))
        state, deleted = self.plan(91, state)
        self.assertEqual(deleted, [])  # Native expiry already removed them.
        self.assertEqual(state["entries"], {})

    def test_old_short_retention_artifacts_are_not_retrofitted(self):
        for artifact in self.artifacts[self.old["id"]]:
            artifact["expires_at"] = r.iso(r.timestamp(artifact["created_at"]) + timedelta(days=14))
        self.assertEqual(self.plan(12)[0]["entries"], {})

    def test_stale_ledger_never_deletes_restored_last_good(self):
        state, _ = self.plan(16)
        self.artifacts[self.new["id"]] = []
        self.assertEqual(self.plan(25, state)[1], [])

    def test_active_old_rerun_is_not_touched(self):
        state, _ = self.plan(16)
        self.old.update(status="in_progress", run_attempt=2)
        self.assertEqual(self.plan(25, state)[1], [])

    def test_malformed_state_identity_and_deadline_abort(self):
        state, _ = self.plan(16)
        key = next(iter(state["entries"]))
        for field, value in [("name", "unrelated"), ("delete_after", "2026-01-01T00:00:00Z"),
                             ("artifact_id", 999), ("branch", "main"), ("head_sha", "bad")]:
            with self.subTest(field=field):
                changed = copy.deepcopy(state)
                changed["entries"][key][field] = value
                with self.assertRaises(ValueError):
                    self.plan(25, changed)

    def test_plan_is_read_only_and_preserves_input(self):
        before = copy.deepcopy(self.state)
        with mock.patch.object(r.subprocess, "run", side_effect=AssertionError("No external calls")):
            self.plan()
        self.assertEqual(self.state, before)

    def fake_api(self):
        api = mock.Mock()
        api.base = "repos/test/repo"
        api.is_descendant.side_effect = ancestry
        api.inventory.return_value = self.runs, self.artifacts, self.jobs
        api.run.side_effect = lambda run_id: copy.deepcopy(next(x for x in self.runs if x["id"] == run_id))
        api.artifacts.side_effect = lambda run_id: copy.deepcopy(self.artifacts[run_id])
        api.jobs.side_effect = lambda item: copy.deepcopy(self.jobs[item["id"]])
        def request(path, method="GET"):
            if method == "DELETE":
                return None
            ident = int(path.rsplit("/", 1)[1])
            return copy.deepcopy(next(a for batch in self.artifacts.values() for a in batch if a["id"] == ident))
        api.request.side_effect = request
        return api

    def test_apply_deletes_only_seven_persisted_eligible_ids(self):
        state, candidates = self.plan()
        api = self.fake_api()
        deleted = r.apply_plan(api, REPO, WORKFLOW, state, BASE + timedelta(days=25))
        self.assertEqual(deleted, [c["artifact_id"] for c in candidates])
        self.assertEqual(sum(call.kwargs.get("method") == "DELETE" for call in api.request.call_args_list), 7)

    def test_apply_refuses_new_unpersisted_decisions(self):
        api = self.fake_api()
        self.assertEqual(r.apply_plan(api, REPO, WORKFLOW, self.state, BASE + timedelta(days=25)), [])
        api.request.assert_not_called()

    def test_apply_aborts_when_target_rerun_starts(self):
        state, _ = self.plan()
        api = self.fake_api()
        api.run.side_effect = lambda ident: {**self.old, "status": "in_progress", "run_attempt": 2}
        with self.assertRaisesRegex(RuntimeError, "Run changed"):
            r.apply_plan(api, REPO, WORKFLOW, state, BASE + timedelta(days=25))
        api.request.assert_not_called()

    def test_apply_aborts_when_replacement_disappears(self):
        state, _ = self.plan()
        api = self.fake_api()
        api.artifacts.side_effect = lambda ident: []
        with self.assertRaisesRegex(RuntimeError, "replacement"):
            r.apply_plan(api, REPO, WORKFLOW, state, BASE + timedelta(days=25))
        api.request.assert_not_called()

    def test_apply_rechecks_clock_if_replacement_expires_during_reads(self):
        state, _ = self.plan()
        api = self.fake_api()
        times = iter([BASE + timedelta(days=25), BASE + timedelta(days=25),
                      BASE + timedelta(days=101)])
        with self.assertRaisesRegex(RuntimeError, "Artifact changed|Replacement expired"):
            r.apply_plan(api, REPO, WORKFLOW, state, BASE + timedelta(days=25), clock=lambda: next(times))
        self.assertFalse(any(c.kwargs.get("method") == "DELETE" for c in api.request.call_args_list))

    def test_apply_aborts_on_changed_artifact_before_delete(self):
        state, _ = self.plan()
        api = self.fake_api()
        api.request.side_effect = lambda *args, **kwargs: {**self.artifacts[self.old["id"]][0], "name": "spoof"}
        with self.assertRaisesRegex(RuntimeError, "Artifact changed"):
            r.apply_plan(api, REPO, WORKFLOW, state, BASE + timedelta(days=25))
        self.assertFalse(any(c.kwargs.get("method") == "DELETE" for c in api.request.call_args_list))


    def test_symbol_and_packages_share_supersession_but_keep_distinct_native_caps(self):
        self.artifacts[self.new["id"]] = artifacts(self.new, 29)
        self.jobs[self.new["id"]] = jobs(self.new, 29)
        state, deleted = self.plan(29.5)
        self.assertEqual(deleted, [])
        self.assertEqual(len(state["entries"]), 7)
        for entry in state["entries"].values():
            self.assertEqual(entry["first_superseded_at"], r.iso(BASE + timedelta(days=29, hours=2)))
            expected = (BASE + timedelta(days=30, hours=1) if "-symbols-" in entry["name"]
                        else BASE + timedelta(days=43, hours=2))
            self.assertEqual(entry["delete_after"], r.iso(expected))
        state, deleted = self.plan(43.5, state)
        self.assertEqual(len(deleted), 6)
        self.assertTrue(all("-symbols-" not in entry["name"] for entry in deleted))

    def test_verified_preview_remains_a_replacement_after_symbols_expire(self):
        state, _ = self.plan(25)
        expected_proof = copy.deepcopy(state["proofs"][r.proof_key(self.new)])
        self.artifacts[self.new["id"]] = [a for a in self.artifacts[self.new["id"]]
                                           if "-symbols-" not in a["name"]]
        state, deleted = self.plan(45, state)
        self.assertEqual(len(deleted), 6)
        self.assertEqual(state["proofs"][r.proof_key(self.new)], expected_proof)
        self.assertEqual({e["first_superseded_at"] for e in deleted},
                         {r.iso(BASE + timedelta(days=10, hours=2))})

    def test_missing_proof_keeps_old_preview_when_replacement_symbols_expired(self):
        state, deleted = self.plan(45)
        self.assertEqual(deleted, [])
        self.assertEqual(state["proofs"], {})

    def test_proof_does_not_excuse_symbols_missing_before_native_expiry(self):
        state, _ = self.plan(16)
        self.artifacts[self.new["id"]] = [a for a in self.artifacts[self.new["id"]]
                                           if "-symbols-" not in a["name"]]
        self.assertEqual(self.plan(25, state)[1], [])

    def test_proof_still_requires_live_matching_six_packages(self):
        state, _ = self.plan(16)
        original = copy.deepcopy(self.artifacts[self.new["id"]])
        mutations = [lambda batch: batch.pop(),
                     lambda batch: batch[-1].update(id=99999999),
                     lambda batch: batch[-1].update(size_in_bytes=0),
                     lambda batch: batch[-1].update(expired=True),
                     lambda batch: batch[-1]["workflow_run"].update(head_sha="f" * 40),
                     lambda batch: batch[-1].update(expires_at=r.iso(BASE + timedelta(days=120)))]
        for mutate in mutations:
            with self.subTest(mutation=mutate):
                self.artifacts[self.new["id"]] = copy.deepcopy(original)
                mutate(self.artifacts[self.new["id"]])
                self.assertEqual(self.plan(45, state)[1], [])

    def test_proof_is_bound_to_run_attempt_branch_commit_and_jobs(self):
        state, _ = self.plan(16)
        original_run, original_jobs = copy.deepcopy(self.new), copy.deepcopy(self.jobs[self.new["id"]])
        for change in [{"run_attempt": 2}, {"head_sha": "f" * 40}, {"workflow_id": 999},
                       {"head_branch": "codex/free-account-localization-macos-smoke"},
                       {"conclusion": "failure"}, {"event": "pull_request"}]:
            with self.subTest(change=change):
                self.new.update(change)
                self.assertEqual(self.plan(45, state)[1], [])
                self.new.clear()
                self.new.update(original_run)
        self.jobs[self.new["id"]][0]["conclusion"] = "failure"
        self.assertEqual(self.plan(45, state)[1], [])
        self.jobs[self.new["id"]] = original_jobs
        self.jobs[self.new["id"]][0]["completed_at"] = r.iso(BASE + timedelta(days=11))
        self.assertEqual(self.plan(45, state)[1], [])

    def test_last_good_keeps_90_day_packages_after_symbol_expiry(self):
        self.runs = [self.old]
        state, _ = self.plan(20)
        self.assertIn(r.proof_key(self.old), state["proofs"])
        state, deleted = self.plan(89, state)
        self.assertEqual(deleted, [])
        self.assertEqual(state["entries"], {})
        self.assertIsNotNone(r.complete_run(self.old, self.artifacts[self.old["id"]],
                                           self.jobs[self.old["id"]], REPO, WORKFLOW,
                                           BASE + timedelta(days=89), state["proofs"][r.proof_key(self.old)]))
        state, deleted = self.plan(91, state)
        self.assertEqual(state["proofs"], {})
        self.assertEqual(deleted, [])

    def test_proof_does_not_restore_deleted_last_good_packages(self):
        state, _ = self.plan(16)
        self.artifacts[self.new["id"]] = []
        self.assertEqual(self.plan(45, state)[1], [])

    def test_malformed_or_incomplete_proof_aborts(self):
        state, _ = self.plan(16)
        key = r.proof_key(self.new)
        mutations = [lambda proof: proof.update(run_attempt=2),
                     lambda proof: proof.update(head_sha="bad"),
                     lambda proof: proof.update(verified_at=r.iso(BASE + timedelta(days=50))),
                     lambda proof: proof["artifacts"].pop("symbols"),
                     lambda proof: proof["artifacts"]["symbols"].update(name="spoof"),
                     lambda proof: proof["artifacts"]["symbols"].update(artifact_id=True),
                     lambda proof: proof["artifacts"]["symbols"].update(expires_at=r.iso(BASE))]
        for mutate in mutations:
            with self.subTest(mutation=mutate):
                changed = copy.deepcopy(state)
                mutate(changed["proofs"][key])
                with self.assertRaises(ValueError):
                    self.plan(45, changed)

    def test_proofs_and_inputs_are_not_mutated_by_later_planning(self):
        state, _ = self.plan(16)
        inputs = copy.deepcopy((self.runs, self.artifacts, self.jobs, state))
        self.plan(45, state)
        self.assertEqual((self.runs, self.artifacts, self.jobs, state), inputs)

    def test_apply_revalidates_persisted_proof_after_symbol_expiry(self):
        state, _ = self.plan(16)
        state, candidates = self.plan(45, state)
        api = self.fake_api()
        deleted = r.apply_plan(api, REPO, WORKFLOW, state, BASE + timedelta(days=45))
        self.assertEqual(deleted, [entry["artifact_id"] for entry in candidates])
        self.assertEqual(len(deleted), 6)

    def test_apply_proof_does_not_override_fresh_missing_package(self):
        state, _ = self.plan(16)
        state, _ = self.plan(45, state)
        api = self.fake_api()
        api.artifacts.side_effect = lambda ident: self.artifacts[ident][:-1]
        with self.assertRaisesRegex(RuntimeError, "replacement"):
            r.apply_plan(api, REPO, WORKFLOW, state, BASE + timedelta(days=45))
        self.assertFalse(any(c.kwargs.get("method") == "DELETE" for c in api.request.call_args_list))

    def test_apply_proof_rechecks_package_expiry_at_delete_boundary(self):
        state, _ = self.plan(16)
        state, _ = self.plan(45, state)
        api = self.fake_api()
        times = iter([BASE + timedelta(days=45), BASE + timedelta(days=45),
                      BASE + timedelta(days=101)])
        with self.assertRaisesRegex(RuntimeError, "Artifact changed|Replacement expired"):
            r.apply_plan(api, REPO, WORKFLOW, state, BASE + timedelta(days=45), clock=lambda: next(times))
        self.assertFalse(any(c.kwargs.get("method") == "DELETE" for c in api.request.call_args_list))

    def test_same_commit_rebuild_never_starts_supersession(self):
        self.new["head_sha"] = self.old["head_sha"]
        for artifact in self.artifacts[self.new["id"]]:
            artifact["workflow_run"]["head_sha"] = self.old["head_sha"]
        for event in ("push", "schedule", "workflow_dispatch"):
            with self.subTest(event=event):
                self.new["event"] = event
                state, deleted = self.plan(25)
                self.assertEqual(state["entries"], {})
                self.assertEqual(deleted, [])
                self.assertEqual(len(state["proofs"]), 2)

    def test_different_commit_schedule_and_manual_build_can_supersede(self):
        for event in ("schedule", "workflow_dispatch"):
            with self.subTest(event=event):
                self.new["event"] = event
                self.assertEqual(len(self.plan(25)[1]), 7)

    def test_apply_rejects_same_commit_rebuild_even_with_stale_ledger(self):
        state, _ = self.plan(25)
        self.new["head_sha"] = self.old["head_sha"]
        for artifact in self.artifacts[self.new["id"]]:
            artifact["workflow_run"]["head_sha"] = self.old["head_sha"]
        api = self.fake_api()
        self.assertEqual(r.apply_plan(api, REPO, WORKFLOW, state, BASE + timedelta(days=25)), [])
        api.request.assert_not_called()

    def test_old_commit_with_newer_run_number_cannot_supersede(self):
        self.old["head_sha"], self.new["head_sha"] = self.new["head_sha"], self.old["head_sha"]
        self.new["event"] = "workflow_dispatch"
        for item in self.runs:
            for artifact in self.artifacts[item["id"]]:
                artifact["workflow_run"]["head_sha"] = item["head_sha"]
        state, deleted = self.plan(25)
        self.assertEqual(state["entries"], {})
        self.assertEqual(deleted, [])

    def test_missing_or_diverged_ancestry_fails_closed(self):
        for check in (None, lambda base, head: False):
            with self.subTest(ancestry=check):
                state, deleted = r.plan(REPO, WORKFLOW, self.runs, self.artifacts, self.jobs,
                                        self.state, BASE + timedelta(days=25), check)
                self.assertEqual(state["entries"], {})
                self.assertEqual(deleted, [])

    def test_first_supersession_ignores_an_earlier_unrelated_success(self):
        later = run(3, 12)
        self.runs.append(later)
        self.artifacts[later["id"]], self.jobs[later["id"]] = artifacts(later, 12), jobs(later, 12)
        check = lambda base, head: head == later["head_sha"] and ancestry(base, head)
        state, _ = r.plan(REPO, WORKFLOW, self.runs, self.artifacts, self.jobs,
                          self.state, BASE + timedelta(days=25), check)
        self.assertEqual({entry["first_superseded_at"] for entry in state["entries"].values()},
                         {r.iso(BASE + timedelta(days=12, hours=2))})

    def test_apply_rechecks_ancestry_after_inventory(self):
        state, _ = self.plan(25)
        api = self.fake_api()
        # Plan verified the relationship; its fresh pre-DELETE check now cannot.
        api.is_descendant.side_effect = [True, False]
        with self.assertRaisesRegex(RuntimeError, "replacement"):
            r.apply_plan(api, REPO, WORKFLOW, state, BASE + timedelta(days=25))
        self.assertFalse(any(c.kwargs.get("method") == "DELETE" for c in api.request.call_args_list))

    def test_replacement_expires_while_late_old_rerun_remains_live(self):
        state, _ = self.plan(16)
        self.old["run_attempt"] = 2
        self.artifacts[self.old["id"]] = artifacts(self.old, 50)
        self.jobs[self.old["id"]] = jobs(self.old, 50)
        state, candidates = self.plan(60, state)
        self.assertEqual(len(candidates), 7)
        api = self.fake_api()
        times = iter([BASE + timedelta(days=60), BASE + timedelta(days=60),
                      BASE + timedelta(days=101)])
        with self.assertRaisesRegex(RuntimeError, "Replacement expired"):
            r.apply_plan(api, REPO, WORKFLOW, state, BASE + timedelta(days=60), clock=lambda: next(times))
        self.assertFalse(any(c.kwargs.get("method") == "DELETE" for c in api.request.call_args_list))

    def test_expired_duplicate_does_not_pollute_verified_proof(self):
        expired = copy.deepcopy(self.artifacts[self.new["id"]][-1])
        expired.update(id=9999999, expired=True)
        self.artifacts[self.new["id"]].append(expired)
        state, _ = self.plan(16)
        proof = state["proofs"][r.proof_key(self.new)]
        self.assertNotIn(expired["id"], [a["artifact_id"] for a in proof["artifacts"].values()])

    def test_old_90_day_symbols_are_not_retrofitted_as_30_day_symbols(self):
        symbol = next(a for a in self.artifacts[self.old["id"]] if "-symbols-" in a["name"])
        symbol["expires_at"] = r.iso(r.timestamp(symbol["created_at"]) + r.MAX_AGE)
        state, deleted = self.plan(25)
        self.assertEqual(len(deleted), 6)
        self.assertNotIn(str(symbol["id"]), state["entries"])


class ClientAndWorkflowTests(unittest.TestCase):
    def test_commit_ancestry_requires_forward_evidence_and_caches_sha_pair(self):
        api = r.GitHub("test/repo")
        base, head = "1" * 40, "2" * 40
        comparison = {"status": "ahead", "ahead_by": 1234, "behind_by": 0,
                      "base_commit": {"sha": base}, "merge_base_commit": {"sha": base}, "commits": []}
        with mock.patch.object(api, "request", return_value=comparison) as request:
            self.assertTrue(api.is_descendant(base, head))
            self.assertTrue(api.is_descendant(base, head))
            request.assert_called_once_with(f"{api.base}/compare/{base}...{head}?per_page=1")
            self.assertFalse(api.is_descendant(base, base))

    def test_same_behind_diverged_or_malformed_comparison_never_qualifies(self):
        base, head = "1" * 40, "2" * 40
        for status in ("behind", "identical", "diverged"):
            with self.subTest(status=status):
                api = r.GitHub("test/repo")
                with mock.patch.object(api, "request", return_value={"status": status, "base_commit": {"sha": base}}):
                    self.assertFalse(api.is_descendant(base, head))
        good = {"status": "ahead", "ahead_by": 1, "behind_by": 0,
                "base_commit": {"sha": base}, "merge_base_commit": {"sha": base}}
        for mutation in ({"status": "unknown"}, {"base_commit": {"sha": head}},
                         {"merge_base_commit": {"sha": head}}, {"ahead_by": True},
                         {"ahead_by": 0}, {"behind_by": 1}, {"behind_by": False}):
            with self.subTest(mutation=mutation):
                api = r.GitHub("test/repo")
                with mock.patch.object(api, "request", return_value=good | mutation):
                    with self.assertRaises(ValueError):
                        api.is_descendant(base, head)

    def test_ancestry_rejects_untrusted_sha_paths_and_api_failure(self):
        api = r.GitHub("test/repo")
        with mock.patch.object(api, "request") as request:
            with self.assertRaises(ValueError):
                api.is_descendant("../other-repository", "f" * 40)
            request.assert_not_called()
        with mock.patch.object(api, "request", side_effect=RuntimeError("compare unavailable")):
            with self.assertRaisesRegex(RuntimeError, "compare unavailable"):
                api.is_descendant("1" * 40, "2" * 40)
        self.assertEqual(api._ancestry, {})

    def test_pagination_reads_more_than_one_page(self):
        api = r.GitHub("test/repo")
        with mock.patch.object(api, "request", side_effect=[{"total_count": 101, "artifacts": [{"id": n + 1} for n in range(100)]},
                                                           {"total_count": 101, "artifacts": [{"id": 101}]}]) as request:
            self.assertEqual(len(api.pages(api.base + "/actions/artifacts", "artifacts")), 101)
            self.assertIn("page=2", request.call_args.args[0])

    def test_truncated_duplicate_and_changing_pagination_fail_closed(self):
        api = r.GitHub("test/repo")
        cases = [[{"total_count": 2, "artifacts": [{"id": 1}]}],
                 [{"total_count": 2, "artifacts": [{"id": 1}, {"id": 1}]}],
                 [{"total_count": 101, "artifacts": [{"id": n + 1} for n in range(100)]},
                  {"total_count": 100, "artifacts": []}]]
        for pages in cases:
            with self.subTest(pages=len(pages)), mock.patch.object(api, "request", side_effect=pages):
                with self.assertRaises(RuntimeError):
                    api.pages(api.base + "/actions/artifacts", "artifacts")

    def test_inventory_only_fetches_runs_with_live_scoped_artifacts(self):
        api = r.GitHub("test/repo")
        item = run(1)
        listing = artifacts(item)
        unrelated = copy.deepcopy(listing[0])
        unrelated["id"] += 1000
        unrelated["name"] = "diagnostic-999-1"
        unrelated["workflow_run"]["id"] = 999
        with mock.patch.object(api, "pages", return_value=listing + [unrelated]), \
             mock.patch.object(api, "run", return_value=item) as fetch, \
             mock.patch.object(api, "jobs", return_value=jobs(item)):
            runs, _, _ = api.inventory(REPO, WORKFLOW, BASE + timedelta(days=1))
            self.assertEqual(runs, [item])
            fetch.assert_called_once_with(item["id"])

    def test_api_writes_are_limited_to_artifact_delete(self):
        api = r.GitHub("test/repo")
        for method, path in [("DELETE", "/actions/runs/1"), ("DELETE", "/releases/1"),
                             ("POST", "/actions/artifacts/1"), ("DELETE", "/actions/artifacts/1/logs")]:
            with self.subTest(path=path), mock.patch.object(r.subprocess, "run") as command:
                with self.assertRaises(ValueError):
                    api.request(api.base + path, method=method)
                command.assert_not_called()

    def test_ledger_latest_artifact_wins_even_old_cleanup_rerun_failed(self):
        api = r.GitHub("test/repo")
        state = r.empty_state(REPO, WORKFLOW)
        listing = [{"id": 10, "name": f"{r.STATE_PREFIX}-100-1", "expired": False,
                    "workflow_run": {"id": 100}},
                   {"id": 20, "name": f"{r.STATE_PREFIX}-90-2", "expired": False,
                    "workflow_run": {"id": 90}}]
        with mock.patch.object(api, "pages", return_value=listing), \
             mock.patch.object(api, "run", return_value=run(3, workflow_id=CLEANUP, path=r.CLEANUP_PATH, event="schedule", conclusion="failure")), \
             mock.patch.object(api, "state_artifact", return_value=state) as read:
            self.assertEqual(api.load_state(REPO, WORKFLOW, CLEANUP), state)
            self.assertEqual(read.call_args.args[0], 20)

    def test_ledger_lookalike_from_other_workflow_ignored(self):
        api = r.GitHub("test/repo")
        listing = [{"id": 20, "name": f"{r.STATE_PREFIX}-90-1", "expired": False,
                    "workflow_run": {"id": 90}}]
        with mock.patch.object(api, "pages", return_value=listing), \
             mock.patch.object(api, "run", return_value={"workflow_id": 999}), \
             mock.patch.object(api, "state_artifact") as read:
            self.assertEqual(api.load_state(REPO, WORKFLOW, CLEANUP), r.empty_state(REPO, WORKFLOW))
            read.assert_not_called()

    def test_same_workflow_untrusted_ledger_lookalikes_are_ignored(self):
        api = r.GitHub("test/repo")
        listing = [{"id": 20, "name": f"{r.STATE_PREFIX}-90-1", "expired": False,
                    "workflow_run": {"id": 90}}]
        valid = run(3, workflow_id=CLEANUP, path=r.CLEANUP_PATH, event="schedule")
        for change in [{"event": "pull_request"}, {"pull_requests": [{"number": 1}]},
                       {"head_repository": {"id": 99}}, {"head_branch": "feature"}]:
            with self.subTest(change=change), mock.patch.object(api, "pages", return_value=listing), \
                 mock.patch.object(api, "run", return_value=valid | change), \
                 mock.patch.object(api, "state_artifact") as read:
                self.assertEqual(api.load_state(REPO, WORKFLOW, CLEANUP), r.empty_state(REPO, WORKFLOW))
                read.assert_not_called()

    def test_zip_allows_only_expected_json_no_path_traversal(self):
        item = run(3, workflow_id=CLEANUP, path=r.CLEANUP_PATH, event="schedule")
        artifact = artifacts(item)[0]
        artifact["name"] = f"{r.STATE_PREFIX}-{item['id']}-1"
        api = r.GitHub("test/repo")
        for filename in ["../state.json", "evil.py", "nested/state.json"]:
            stream = io.BytesIO()
            with zipfile.ZipFile(stream, "w") as archive:
                archive.writestr(filename, "{}")
            with mock.patch.object(api, "request", side_effect=[artifact, stream.getvalue()]), \
                 mock.patch.object(api, "run", return_value=item):
                with self.assertRaisesRegex(ValueError, "archive contents"):
                    api.state_artifact(artifact["id"], REPO, CLEANUP)

    def test_cli_defaults_to_plan_and_never_applies(self):
        api = mock.Mock()
        api.request.return_value = REPO
        api.workflow.side_effect = [WORKFLOW, CLEANUP]
        api.load_state.return_value = r.empty_state(REPO, WORKFLOW)
        api.inventory.return_value = [], {}, {}
        with tempfile.TemporaryDirectory() as directory, \
             mock.patch.object(r, "GitHub", return_value=api), \
             mock.patch.object(r, "apply_plan") as apply, \
             mock.patch.object(sys, "argv", ["preview-retention.py", "--repository", "test/repo",
                                             "--output", directory]), \
             mock.patch.dict(r.os.environ, {}, clear=True), \
             mock.patch("sys.stdout", new_callable=io.StringIO):
            r.main()
            self.assertTrue((Path(directory) / "state.json").exists())
            self.assertTrue((Path(directory) / "report.json").exists())
            apply.assert_not_called()
            self.assertFalse(any(c.kwargs.get("method") == "DELETE" for c in api.request.call_args_list))

    def test_apply_cli_requires_persisted_state_artifact(self):
        api = mock.Mock()
        api.request.return_value = REPO
        api.workflow.side_effect = [WORKFLOW, CLEANUP]
        with mock.patch.object(r, "GitHub", return_value=api), \
             mock.patch.object(r, "apply_plan") as apply, \
             mock.patch.object(sys, "argv", ["preview-retention.py", "--repository", "test/repo", "--apply"]), \
             mock.patch("sys.stderr", new_callable=io.StringIO):
            with self.assertRaises(SystemExit):
                r.main()
            apply.assert_not_called()
            api.state_artifact.assert_not_called()

    def test_cleanup_workflow_has_trusted_checkout_read_only_default_and_persist_first(self):
        text = (ROOT / r.CLEANUP_PATH).read_text(encoding="utf-8")
        self.assertIn("ref: ${{ github.sha }}", text)
        self.assertNotIn("ref: ${{ github.event.workflow_run.head_sha }}", text)
        self.assertIn("github.ref == 'refs/heads/zh-dev'", text)
        self.assertIn("default: true", text)
        self.assertIn("cancel-in-progress: false", text)
        self.assertIn("--state-artifact-id \"$STATE_ARTIFACT_ID\"", text)
        self.assertLess(text.index("id: state"), text.index("--apply"))
        self.assertNotIn("pull_request_target:", text)
        ci = (ROOT / r.CI_PATH).read_text(encoding="utf-8")
        self.assertNotIn("actions: write", ci)


if __name__ == "__main__":
    unittest.main()
