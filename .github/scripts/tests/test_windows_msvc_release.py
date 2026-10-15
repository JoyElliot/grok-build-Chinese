"""Exercise the default-off single-archive policy and native-only retirement."""

import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
import urllib.request
from unittest.mock import patch

from test_release_workflow import dependencies, job_blocks

ROOT = Path(__file__).resolve().parents[3]
spec = importlib.util.spec_from_file_location("msvc_release", ROOT / ".github/scripts/windows-msvc-release.py")
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)
VERSION, TAG = "1.0.99", "release-v1.0.99"


def policy(enabled=True, distribution="embedded", pin=None):
    return {"publish_legacy_sha256": False, "legacy_bridge_tag": "release-v1.0.45",
            "windows_x64_msvc": {"enabled": enabled, "distribution": distribution,
                                 "final_legacy_wrapper": pin}}


def wrapper_fixture():
    data = b"authenticated final wrapper fixture"
    pin = {"version": "1.0.98", "size": len(data), "sha256": hashlib.sha256(data).hexdigest()}
    name = "grok-zh-1.0.98-windows-x86_64-gnu.zip"
    metadata = {"tag_name": "release-v1.0.98", "immutable": True, "draft": False, "prerelease": False,
                "assets": [{"name": name, "state": "uploaded", "size": pin["size"],
                            "digest": "sha256:" + pin["sha256"],
                            "browser_download_url": f"https://github.com/{release.REPOSITORY}/releases/download/release-v1.0.98/{name}"}]}
    return pin, data, metadata


class WindowsMsvcReleaseTests(unittest.TestCase):
    def test_disabled_policy_keeps_legacy_versions_and_needs_no_network(self):
        with patch.object(release, "fetch_bytes") as fetch:
            for version, tag in (("1.0.8", "v1.0.8"), (VERSION, TAG), ("1.0.99-alpha.1", "release-v1.0.99-alpha.1")):
                plan = release.preflight(policy(False), version, tag, release.REPOSITORY)
                self.assertEqual(plan["windows_msvc_enabled"], "false")
                self.assertEqual(plan["windows_x64_distribution"], "gnu")
                self.assertEqual(plan["windows_public_archive_name"], f"grok-zh-{version}-windows-x86_64-gnu.zip")
            fetch.assert_not_called()

    def test_embedded_mode_has_one_legacy_named_public_archive_without_helper(self):
        with patch.object(release, "fetch_bytes") as fetch:
            plan = release.preflight(policy(), VERSION, TAG, release.REPOSITORY)
            self.assertEqual(plan["windows_msvc_enabled"], "true")
            self.assertEqual(plan["windows_x64_distribution"], "embedded")
            self.assertEqual(plan["windows_public_archive_name"], f"grok-zh-{VERSION}-windows-x86_64-gnu.zip")
            self.assertNotEqual(plan["windows_public_archive_name"], plan["msvc_archive_name"])
            fetch.assert_not_called()

    def test_checked_in_policy_is_valid_in_any_reviewed_state(self):
        checked = json.loads((ROOT / ".github/release-policy.json").read_text(encoding="utf-8"))
        pin = checked["windows_x64_msvc"].get("final_legacy_wrapper")
        version = VERSION
        if pin:
            major, minor, patch_number = release.stable_version(pin["version"])
            version = f"{major}.{minor}.{patch_number + 1}"
        plan = release.plan_release(checked, version, f"release-v{version}", release.REPOSITORY)
        self.assertEqual(plan["windows_msvc_enabled"], str(checked["windows_x64_msvc"]["enabled"]).lower())

    def test_enabled_requires_official_stable_version_digest_policy(self):
        for version, tag, repository in (("1.0.45", "release-v1.0.45", release.REPOSITORY),
                                         (VERSION, "v" + VERSION, release.REPOSITORY),
                                         ("1.0.99-alpha.1", "release-v1.0.99-alpha.1", release.REPOSITORY),
                                         (VERSION, TAG, "other/repository")):
            with self.subTest(version=version, tag=tag), self.assertRaises(ValueError):
                release.plan_release(policy(), version, tag, repository)
        with self.assertRaises(ValueError):
            release.plan_release(policy() | {"publish_legacy_sha256": True}, VERSION, TAG, release.REPOSITORY)

    def test_invalid_policy_fails_closed(self):
        for config in ({"enabled": False}, {"enabled": True, "helper": None},
                       {"enabled": "true", "distribution": "embedded", "final_legacy_wrapper": None},
                       {"enabled": True, "distribution": "both", "final_legacy_wrapper": None},
                       {"enabled": True, "distribution": "native", "final_legacy_wrapper": None}):
            with self.subTest(config=config), self.assertRaises(ValueError):
                release.plan_release(policy() | {"windows_x64_msvc": config}, VERSION, TAG, release.REPOSITORY)

    def test_native_retirement_requires_older_exact_final_wrapper(self):
        pin, _, _ = wrapper_fixture()
        plan = release.plan_release(policy(distribution="native", pin=pin), VERSION, TAG, release.REPOSITORY)
        self.assertEqual(plan["windows_public_archive_name"], plan["msvc_archive_name"])
        for key, value in (("version", VERSION), ("version", "1.0.45"), ("version", "latest"),
                           ("size", True), ("size", 0), ("size", release.MAX_WRAPPER_BYTES + 1),
                           ("sha256", "pending"), ("extra", 0)):
            with self.subTest(key=key), self.assertRaises(ValueError):
                release.plan_release(policy(distribution="native", pin=pin | {key: value}), VERSION, TAG, release.REPOSITORY)

    def test_retirement_release_is_immutable_and_asset_is_unambiguous(self):
        pin, _, metadata = wrapper_fixture()
        self.assertEqual(release.validate_wrapper_release(metadata, pin, release.REPOSITORY),
                         metadata["assets"][0]["browser_download_url"])
        for key, value in (("immutable", False), ("immutable", 1), ("draft", True),
                           ("prerelease", True), ("tag_name", TAG), ("assets", []),
                           ("assets", metadata["assets"] * 2)):
            with self.subTest(key=key), self.assertRaises(ValueError):
                release.validate_wrapper_release(metadata | {key: value}, pin, release.REPOSITORY)
        for key, value in (("state", "new"), ("size", pin["size"] + 1),
                           ("digest", "sha256:" + "0" * 64), ("browser_download_url", "https://example.org/wrapper.zip")):
            changed = copy.deepcopy(metadata)
            changed["assets"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                release.validate_wrapper_release(changed, pin, release.REPOSITORY)

    def test_retirement_authenticates_bytes_and_embedded_contract(self):
        pin, data, metadata = wrapper_fixture()
        with patch.object(release, "fetch_bytes", side_effect=[json.dumps(metadata).encode(), data]) as fetch, \
                patch.object(release, "bootstrap_builder") as factory:
            release.preflight(policy(distribution="native", pin=pin), VERSION, TAG, release.REPOSITORY)
            factory.return_value.validate_compat_archive.assert_called_once()
            self.assertEqual(factory.return_value.validate_compat_archive.call_args.args[1], pin["version"])
            self.assertEqual(fetch.call_args_list[1].args, (metadata["assets"][0]["browser_download_url"], pin["size"]))
        for wrong in (data + b"extra", b"x" * len(data)):
            with patch.object(release, "fetch_bytes", side_effect=[json.dumps(metadata).encode(), wrong]), \
                    patch.object(release, "bootstrap_builder") as factory, self.assertRaises(ValueError):
                release.preflight(policy(distribution="native", pin=pin), VERSION, TAG, release.REPOSITORY)
            factory.assert_not_called()
        with patch.object(release, "fetch_bytes", side_effect=[json.dumps(metadata).encode(), data]), \
                patch.object(release, "bootstrap_builder") as factory:
            factory.return_value.validate_compat_archive.side_effect = ValueError("plain GNU is not a wrapper")
            with self.assertRaisesRegex(ValueError, "plain GNU"):
                release.preflight(policy(distribution="native", pin=pin), VERSION, TAG, release.REPOSITORY)

    def test_retirement_accepts_real_verified_wrapper_not_only_metadata(self):
        import test_embedded_msvc_bundle
        with tempfile.TemporaryDirectory() as directory:
            payload, output, _, _, _ = test_embedded_msvc_bundle.EmbeddedMsvcBundleTests().fixture(Path(directory))
            builder = release.bootstrap_builder()
            archive = builder.build_compat_package(output, payload, VERSION, release=True)
            data = archive.read_bytes()
            pin = {"version": VERSION, "size": len(data), "sha256": hashlib.sha256(data).hexdigest()}
            metadata = {"tag_name": TAG, "immutable": True, "draft": False, "prerelease": False,
                        "assets": [{"name": archive.name, "state": "uploaded", "size": len(data),
                                    "digest": "sha256:" + pin["sha256"],
                                    "browser_download_url": f"https://github.com/{release.REPOSITORY}/releases/download/{TAG}/{archive.name}"}]}
            with patch.object(release, "fetch_bytes", side_effect=[json.dumps(metadata).encode(), data]):
                result = release.preflight(policy(distribution="native", pin=pin), "1.0.100", "release-v1.0.100", release.REPOSITORY)
            self.assertEqual(result["windows_x64_distribution"], "native")

    def test_redirects_never_leak_token_or_downgrade_https(self):
        handler = release.SafeRedirectHandler()
        request = urllib.request.Request("https://api.github.com/repos/repo", headers={"Authorization": "Bearer fixture"})
        for url in ("https://example.org/steal", "http://api.github.com/repos/repo", "https://api.github.com:8443/repo",
                    "https://user:password@api.github.com/repo"):
            with self.subTest(url=url), self.assertRaises(ValueError):
                handler.redirect_request(request, None, 302, "Found", {}, url)
        redirected = handler.redirect_request(request, None, 302, "Found", {}, "https://api.github.com/repos/renamed")
        self.assertEqual(redirected.get_header("Authorization"), "Bearer fixture")
        public = urllib.request.Request("https://github.com/release")
        redirected = handler.redirect_request(public, None, 302, "Found", {}, "https://release-assets.githubusercontent.com/asset")
        self.assertFalse(redirected.has_header("Authorization"))

    def test_build_embeds_validated_native_archive_once_with_no_remote_helper(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output, payload, archive = root / "output", root / "payload", root / "native.zip"
            with patch.object(release, "fetch_bytes") as fetch, patch.object(release, "bootstrap_builder") as factory:
                release.build_bridge(policy(), VERSION, TAG, release.REPOSITORY, output, payload, archive, "gcc")
                builder = factory.return_value
                builder.validate_native_archive.assert_called_once_with(archive, VERSION)
                builder.build_launcher.assert_called_once_with(output, VERSION, archive, "gcc")
                builder.build_compat_package.assert_called_once_with(output, payload, VERSION, release=True)
                fetch.assert_not_called()
            self.assertEqual(list(output.iterdir()), [])  # No pin or helper ZIP generated.
            with patch.object(release, "bootstrap_builder"), self.assertRaises(FileExistsError):
                release.build_bridge(policy(), VERSION, TAG, release.REPOSITORY, output, payload, archive, "gcc")

    def test_disabled_or_bad_payload_build_creates_no_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.object(release, "bootstrap_builder") as factory, self.assertRaises(ValueError):
                release.build_bridge(policy(False), VERSION, TAG, release.REPOSITORY, root / "out", root / "payload", root / "zip", "gcc")
            factory.assert_not_called()
            with patch.object(release, "bootstrap_builder") as factory:
                factory.return_value.validate_native_archive.side_effect = ValueError("wrong architecture")
                with self.assertRaises(ValueError):
                    release.build_bridge(policy(), VERSION, TAG, release.REPOSITORY, root / "out", root / "payload", root / "zip", "gcc")
                factory.return_value.build_launcher.assert_not_called()
            self.assertFalse((root / "out").exists())


class WindowsMsvcReleaseWorkflowTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.text = (ROOT / ".github/workflows/zh-release-windows.yml").read_text(encoding="utf-8")
        cls.jobs = job_blocks(cls.text)

    def test_guarded_path_and_native_acceptance_are_mandatory(self):
        plan = self.jobs["release-plan"]
        self.assertIn("windows-msvc-release.py preflight", plan)
        self.assertIn("windows_x64_distribution = $msvcPlan.windows_x64_distribution", plan)
        self.assertIn("outputs.windows_msvc_enabled == 'false'", self.jobs["windows-x64-gnu"])
        for name in ("windows-x64-msvc-validation", "windows-x64-msvc-cross-build", "windows-x64-msvc-release"):
            self.assertIn("outputs.windows_msvc_enabled == 'true'", self.jobs[name])
            self.assertIn("ref: ${{ needs.release-plan.outputs.source_commit }}", self.jobs[name])
            self.assertNotIn("continue-on-error", self.jobs[name])
        test = self.jobs["windows-x64-msvc-validation"]
        self.assertIn("validation-mode: release", test)
        self.assertIn("phase: test", test)
        build = self.jobs["windows-x64-msvc-release"]
        self.assertEqual(dependencies(build), {"release-plan", "windows-x64-msvc-cross-build"})
        self.assertIn("msvc-build-input.py verify", build)
        self.assertIn("outputs.windows_x64_distribution == 'embedded'", build)
        self.assertIn('--payload-archive "dist/$env:ARCHIVE_NAME"', build)
        self.assertEqual(build.count("Test-EmbeddedMigrationProduct.ps1"), 2)
        self.assertLess(build.index("windows-msvc-release.py build-bridge"), build.index("Test-EmbeddedMigrationProduct.ps1"))
        self.assertLess(build.index("Test-EmbeddedMigrationProduct.ps1"), build.index("uses: actions/upload-artifact"))

    def test_internal_native_payload_is_attested_but_not_a_second_public_archive(self):
        attest, publish = self.jobs["release-attestations"], self.jobs["release-publisher"]
        self.assertIn("outputs.windows_x64_distribution != 'native'", attest)
        for archive in ("ZIP_NAME", "MSVC_ARCHIVE_NAME"):
            self.assertIn("dist/${{ env." + archive + " }}\n", attest)
            self.assertIn("Assert-ChecksumSidecar (Join-Path $dist $env:" + archive + ")", publish)
        self.assertIn("$attestationAssets = @($assets)", publish)
        self.assertIn("foreach ($assetPath in $attestationAssets)", publish)
        self.assertIn("$env:WINDOWS_X64_DISTRIBUTION -eq 'embedded'", publish)
        self.assertIn("(Split-Path $_ -Leaf) -cne $env:MSVC_ARCHIVE_NAME", publish)
        self.assertIn("$env:WINDOWS_PUBLIC_ARCHIVE_NAME }).Count -ne 1", publish)
        self.assertLess(publish.index("$expectedLocalNames"), publish.index("$attestationAssets ="))
        self.assertLess(publish.index("(Split-Path $_ -Leaf) -cne $env:MSVC_ARCHIVE_NAME"), publish.index("'release', 'create'"))
        self.assertLess(publish.index("windows-msvc-release.py preflight"), publish.index("'release', 'create'"))
        self.assertNotIn("migration-pin.json", publish)

    def test_disabled_path_still_runs_policy_tests_without_toggle_override(self):
        checks = (ROOT / ".github/scripts/check-windows-package.ps1").read_text(encoding="utf-8-sig")
        self.assertIn("'test_windows_msvc_release.py'", checks)
        self.assertNotIn("workflow_dispatch", self.text)
        self.assertNotIn("vars.WINDOWS", self.text)


class WindowsMsvcReleaseEncodingTests(unittest.TestCase):
    def test_source_reads_work_with_windows_legacy_default_encoding(self):
        original_read_text = Path.read_text

        def legacy_read_text(path, encoding=None, errors=None):
            return original_read_text(path, encoding=encoding or "cp1252", errors=errors)

        # Exercise the real policy/workflow readers with Windows' legacy
        # default, even on a UTF-8 Linux host. Explicit encodings pass through.
        suite = unittest.TestSuite([
            WindowsMsvcReleaseTests("test_checked_in_policy_is_valid_in_any_reviewed_state"),
            unittest.defaultTestLoader.loadTestsFromTestCase(WindowsMsvcReleaseWorkflowTests),
        ])
        result = unittest.TestResult()
        with patch.object(Path, "read_text", legacy_read_text):
            suite.run(result)
        self.assertEqual(result.testsRun, 4)
        self.assertTrue(result.wasSuccessful(), result.errors + result.failures)


if __name__ == "__main__":
    unittest.main()
