// Historical-only negative regression. A passing assertion reproduces the
// immutable writer's limitation; it does not certify migration safety.
#[tokio::test]
async fn delayed_legacy_writer_overwrites_newer_image_after_lock_release() {
    use std::os::windows::fs::OpenOptionsExt;

    let dir = tempfile::tempdir().unwrap();
    let native = required_input("GROK_DIRECT_NATIVE_EXE");
    let historical = required_input("GROK_DIRECT_HISTORICAL_EXE");
    assert_eq!(path_hash(&native), NEW_HASH);
    assert_eq!(path_hash(&historical), OLD_HASH);
    let package = dir.path().join("installation");
    copy_tree(native.parent().unwrap(), &package);
    let destination = package.join("grok-zh.exe");
    let before = snapshot(&package);
    assert_eq!(path_hash(&destination), NEW_HASH);
    assert!(community_version_output_matches(
        &smoke_test_binary(&destination).await.unwrap(),
        VERSION,
    ));

    // The scheduling seam begins AFTER candidate verification. Real immutable
    // input bytes are used, not a patched version string or invented product.
    // This does not claim that 1.0.45's automatic selection chooses 1.0.45.
    let candidate = dir.path().join("delayed-candidate.exe");
    std::fs::copy(&historical, &candidate).unwrap();
    assert_eq!(path_hash(&candidate), OLD_HASH);
    assert!(community_version_output_matches(
        &smoke_test_binary(&candidate).await.unwrap(),
        "1.0.45",
    ));

    // A newer writer's exclusive lock prevents simultaneous activation, but
    // does not fence a delayed historical writer after that lock is released.
    let newer_lock = acquire_windows_update_lock(&destination).unwrap();
    assert!(acquire_windows_update_lock(&destination).is_err());
    drop(newer_lock);

    // A reader of the newer image is not a permanent downgrade barrier: the
    // original Windows fallback can rename an image with DELETE sharing.
    // This is an open file handle, deliberately NOT called a running process.
    let mut newer_image = OpenOptions::new()
        .read(true)
        .share_mode(0x1 | 0x4)
        .open(&destination)
        .unwrap();
    let delayed_lock = acquire_windows_update_lock(&destination).unwrap();
    windows_replace_exe_locked(&candidate, &destination)
        .await
        .unwrap();
    drop(delayed_lock);

    assert_eq!(path_hash(&destination), OLD_HASH);
    assert!(community_version_output_matches(
        &smoke_test_binary(&destination).await.unwrap(),
        "1.0.45",
    ));
    let mut reader_hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = newer_image.read(&mut buffer).unwrap();
        if count == 0 {
            break;
        }
        reader_hash.update(&buffer[..count]);
    }
    assert_eq!(format!("{:x}", reader_hash.finalize()), NEW_HASH);
    drop(newer_image);
    unchanged_companions(&destination, &before);

    result_file(
        "delayed-writer-overwrite",
        json!({
            "status": "PASSED",
            "risk_reproduced": true,
            "initial_sha256": NEW_HASH,
            "installed_sha256": OLD_HASH,
            "initial_version": VERSION,
            "installed_version": "1.0.45",
            "newer_open_handle_sha256": NEW_HASH,
            "safeguards_current_only": true,
            "whole_immutable_old_executable_update": false,
            "live_https": false,
            "not_a_running_old_process": true,
            "automatic_candidate_selection_tested": false,
            "scope": "verbatim historical lock and activation functions after real candidate verification; newer writer lock released first",
            "interpretation": "negative regression: historical writer replaces a newer installation; PASSED does not mean the risk is fixed"
        }),
    );
}
