// Current-only, self-contained Windows tests. The executing image is a copy of
// THIS Rust test executable, not a fake product build or the immutable GNU EXE.
// Production activation functions are extracted verbatim by prepare.py.
mod windows_atomic_activation_process_tests {
    use super::*;
    use std::os::windows::fs::OpenOptionsExt;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    fn test_name(name: &str) -> String {
        // module_path! includes the crate name; libtest's --exact names do not.
        let module = module_path!().split_once("::").unwrap().1;
        format!("{module}::{name}")
    }
    struct BoundedChild(Child);
    impl Drop for BoundedChild {
        fn drop(&mut self) {
            if self.0.try_wait().ok().flatten().is_none() {
                let _ = self.0.kill();
            }
        }
    }
    fn wait_for_exit(child: &mut BoundedChild) -> std::process::ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Some(status) = child.0.try_wait().unwrap() { return status; }
            assert!(Instant::now() < deadline, "child did not exit within 20 seconds");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    fn result(case: &str, evidence: serde_json::Value) {
        if std::env::var_os("GROK_TRANSACTION_RESULTS").is_some() {
            result_file(case, evidence);
        }
    }
    struct Fixture {
        dest: PathBuf,
        source: PathBuf,
        marker: PathBuf,
        log: PathBuf,
        old: PathBuf,
        original_hash: String,
        replacement_hash: String,
        _locked_old: Option<File>,
        // Drop held handles before the directory attempts Windows cleanup.
        _dir: tempfile::TempDir,
    }
    impl Fixture {
        fn new(locked_old: bool) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let installation = dir.path().join("安装目录 with spaces");
            std::fs::create_dir(&installation).unwrap();
            let dest = installation.join("grok-zh.exe");
            let source = dir.path().join("candidate.exe");
            let current = std::env::current_exe().unwrap();
            std::fs::copy(&current, &dest).unwrap();
            std::fs::copy(&current, &source).unwrap();
            // PE overlay changes byte identity without changing the compiled
            // program. This is explicitly a test image, not a product version.
            OpenOptions::new().append(true).open(&source).unwrap()
                .write_all(b"\natomic-activation-test-replacement\n").unwrap();
            OpenOptions::new().write(true).open(&source).unwrap().sync_all().unwrap();
            let old = installation.join("grok-zh.exe.old");
            let locked = if locked_old {
                std::fs::write(&old, b"unrelated locked prior backup").unwrap();
                Some(OpenOptions::new().read(true).share_mode(1).open(&old).unwrap())
            } else { None };
            let original_hash = path_hash(&dest);
            let replacement_hash = path_hash(&source);
            assert_ne!(original_hash, replacement_hash);
            Self { dest, source, marker: dir.path().join("checkpoint.ready"),
                log: dir.path().join("child.log"), old, original_hash, replacement_hash,
                _locked_old: locked, _dir: dir }
        }
        fn launch(&self, phase: &str, mode: &str) -> BoundedChild {
            let log = File::create(&self.log).unwrap();
            let mut child = BoundedChild(Command::new(&self.dest)
                .args(["--exact", &test_name("activation_child"), "--nocapture"])
                .env("GROK_TEST_ACTIVATION_PHASE", phase)
                .env("GROK_TEST_ACTIVATION_DEST", &self.dest)
                .env("GROK_TEST_ACTIVATION_SOURCE", &self.source)
                .env("GROK_TEST_ACTIVATION_MARKER", &self.marker)
                .env("GROK_TEST_ACTIVATION_MODE", mode)
                .stdin(Stdio::null()).stdout(log.try_clone().unwrap()).stderr(log)
                .spawn().unwrap());
            let deadline = Instant::now() + Duration::from_secs(20);
            loop {
                if std::fs::read_to_string(&self.marker).ok().as_deref() == Some(phase) { break; }
                if let Some(status) = child.0.try_wait().unwrap() {
                    panic!("child exited before checkpoint ({status}): {}", std::fs::read_to_string(&self.log).unwrap());
                }
                assert!(Instant::now() < deadline, "child checkpoint timeout: {}", std::fs::read_to_string(&self.log).unwrap());
                std::thread::sleep(Duration::from_millis(10));
            }
            child
        }
        fn assert_launchable(&self, expected_hash: &str) {
            assert_eq!(path_hash(&self.dest), expected_hash);
            let probe_marker = self.marker.with_extension("probe");
            let _ = std::fs::remove_file(&probe_marker);
            let mut probe = BoundedChild(Command::new(&self.dest)
                .args(["--exact", &test_name("entry_probe")])
                .env("GROK_TEST_ENTRY_PROBE_MARKER", &probe_marker)
                .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
                .spawn().expect("public installed EXE must launch directly, without any repair helper"));
            assert!(wait_for_exit(&mut probe).success());
            assert_eq!(std::fs::read(&probe_marker).unwrap(), b"entry-probe-ran",
                "a zero-test libtest run must not masquerade as a launch probe");
        }
        fn assert_original_backup(&self) {
            assert!(std::fs::read_dir(self.dest.parent().unwrap()).unwrap().any(|entry| {
                let entry = entry.unwrap();
                entry.file_name().to_string_lossy().starts_with("grok-zh.exe.old")
                    && path_hash(&entry.path()) == self.original_hash
            }), "mapped original image must retain a backup name");
        }
    }

    #[test]
    fn activation_child() {
        if std::env::var_os("GROK_TEST_ACTIVATION_MODE").is_none() { return; }
        let dest = required_input("GROK_TEST_ACTIVATION_DEST");
        let source = required_input("GROK_TEST_ACTIVATION_SOURCE");
        assert_eq!(std::fs::canonicalize(std::env::current_exe().unwrap()).unwrap(), std::fs::canonicalize(&dest).unwrap());
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            if std::env::var("GROK_TEST_ACTIVATION_MODE").unwrap() == "pre-fix" {
                let _lock = acquire_windows_update_lock(&dest).unwrap();
                let stage = dest.with_file_name("regression-stage.exe");
                std::fs::copy(&source, &stage).unwrap();
                OpenOptions::new().write(true).open(&stage).unwrap().sync_all().unwrap();
                pre_fix_activation::activate(&stage, &dest, &dest.with_file_name("grok-zh.exe.old")).await.unwrap();
            } else {
                windows_replace_exe(&source, &dest).await.unwrap();
            }
        });
    }
    #[test]
    fn entry_probe() {
        if let Some(marker) = std::env::var_os("GROK_TEST_ENTRY_PROBE_MARKER") {
            std::fs::write(marker, b"entry-probe-ran").unwrap();
        }
    }

    #[test]
    fn live_self_update_preserves_running_image_and_locked_backups() {
        for locked_old in [false, true] {
            let fixture = Fixture::new(locked_old);
            let mut child = fixture.launch("after-replace", "current");
            // The copied installed EXE itself executed the current updater and
            // is STILL mapped/running here. A file-share lock is not a substitute.
            assert!(child.0.try_wait().unwrap().is_none());
            fixture.assert_original_backup();
            fixture.assert_launchable(&fixture.replacement_hash);
            assert!(acquire_windows_update_lock(&fixture.dest).is_err());
            std::fs::write(fixture.marker.with_extension("continue"), b"resume").unwrap();
            assert!(wait_for_exit(&mut child).success(), "{}", std::fs::read_to_string(&fixture.log).unwrap());
            let _lock = acquire_windows_update_lock(&fixture.dest).unwrap();
            tokio::runtime::Runtime::new().unwrap().block_on(sweep_old_exe_backups(&fixture.old));
            if locked_old {
                assert_eq!(std::fs::read(&fixture.old).unwrap(), b"unrelated locked prior backup");
            } else { assert!(!fixture.old.exists()); }
            assert!(!std::fs::read_dir(fixture.dest.parent().unwrap()).unwrap().any(|entry| {
                entry.unwrap().file_name().to_string_lossy().starts_with("grok-zh.exe.old.")
            }), "unlocked unique backups must be cleaned after the mapped child exits");
        }
        result("atomic-live-self-update", json!({"status":"PASSED","live_mapped_self_update":true,"locked_prior_backup_cases":2,"public_entry_direct_launch":true,"image":"copied current Rust transaction test EXE with PE overlay candidate; not product version upgrade","power_loss_tested":false}));
    }

    #[test]
    fn forced_process_kill_never_removes_public_executable() {
        let mut cases = Vec::new();
        for locked_old in [false, true] {
            for phase in ["before-backup", "after-backup", "after-replace"] {
                let fixture = Fixture::new(locked_old);
                let mut child = fixture.launch(phase, "current");
                let expected = if phase == "after-replace" { &fixture.replacement_hash } else { &fixture.original_hash };
                fixture.assert_launchable(expected);
                assert!(acquire_windows_update_lock(&fixture.dest).is_err());
                // std::process::Child::kill uses TerminateProcess on Windows;
                // no Rust destructor, panic rollback or completion path executes.
                child.0.kill().unwrap();
                assert!(!wait_for_exit(&mut child).success());
                fixture.assert_launchable(expected);
                let lock = acquire_windows_update_lock(&fixture.dest).unwrap();
                drop(lock);
                // Prove a subsequent normal update works without an external
                // entry-repair operation or asking the absent EXE to heal itself.
                tokio::runtime::Runtime::new().unwrap().block_on(windows_replace_exe(&fixture.source, &fixture.dest)).unwrap();
                fixture.assert_launchable(&fixture.replacement_hash);
                if locked_old { assert_eq!(std::fs::read(&fixture.old).unwrap(), b"unrelated locked prior backup"); }
                cases.push(json!({"phase":phase,"locked_prior_backup":locked_old,"installed_sha256_after_kill":expected,"entry_present_and_launchable":true,"next_update_succeeded":true}));
            }
        }
        result("atomic-process-kill", json!({"status":"PASSED","cases":cases,"process_kill":"TerminateProcess","power_loss_tested":false,"image":"copied current Rust transaction test EXE, not immutable GNU product"}));
    }

    #[test]
    fn pre_fix_process_kill_reproduces_missing_executable() {
        let fixture = Fixture::new(false);
        let mut child = fixture.launch("after-backup", "pre-fix");
        assert!(!fixture.dest.exists(), "negative fixture must reach the actual missing-entry interval");
        assert_eq!(path_hash(&fixture.old), fixture.original_hash);
        child.0.kill().unwrap();
        assert!(!wait_for_exit(&mut child).success());
        assert!(!fixture.dest.exists());
        assert_eq!(path_hash(&fixture.old), fixture.original_hash);
        result("pre-fix-process-kill", json!({"status":"PASSED","risk_reproduced":true,"fixed_behavior":false,"source_commit":"b3ca1daa","helper_sha256":"02a7a959e6558cf1a006c6c68162e6a181dcd4787e0aeaae6ef91a10dfe51b8e","injection":"test-only checkpoint after successful first rename; saved original helper bytes hash-verified","active_exe_missing":true,"backup_preserved":true,"power_loss_tested":false}));
    }
}
