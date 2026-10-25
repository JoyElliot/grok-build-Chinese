// Current-only tests. These do not claim a distinct future product build.
#[tokio::test]
async fn current_source_accepts_direct_transport_after_migration() {
    let dir=tempfile::tempdir().unwrap();let (destination,before)=genuine_copy(dir.path());
    isolated_transaction(fixture_asset(true),destination.clone(),&input_archive(true),false).await.unwrap();
    assert_eq!(path_hash(&destination),NEW_HASH);unchanged_companions(&destination,&before);
    assert!(community_version_output_matches(&smoke_test_binary(&destination).await.unwrap(),VERSION));
    result_file("direct-reinstall",json!({"status":"PASSED","version":VERSION,"same_version_only":true,"installed_sha256":NEW_HASH}));
}
#[tokio::test]
async fn current_source_activation_failure_restores_real_native_image() {
    let dir=tempfile::tempdir().unwrap();let (destination,before)=genuine_copy(dir.path());
    let old=destination.with_file_name("grok-zh.exe.old");let _lock=acquire_windows_update_lock(&destination).unwrap();
    let result=windows_activate_staged_exe(&dir.path().join("deliberately-missing-stage.exe"),&destination,&old).await;
    assert!(result.is_err());assert_rejection_unchanged(&destination,&before);assert!(!old.exists());
    assert!(community_version_output_matches(&smoke_test_binary(&destination).await.unwrap(),VERSION));
    result_file("activation-rollback",json!({"status":"PASSED","restored_sha256":NEW_HASH,"failure_injection":"missing stage passed to original activation helper, after backup move"}));
}
