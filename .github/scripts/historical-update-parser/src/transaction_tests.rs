// Test-only transport, destination, fixture accounting and assertions.
// Production parser/download/probe/activation bodies are generated verbatim above.
use serde_json::json;
use std::path::PathBuf;
use std::io::Seek;

const OLD_HASH: &str = "474918134a312ab5b2200e03092c5f34a99b5f2d3847057fe827e3ff193404ca";
const NEW_HASH: &str = "8626e60c908f44848190adc9f3547241c651ed8179601d93e33b91469dc76f20";

fn fixture(name: &str) -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name) }
fn required_input(name: &str) -> PathBuf { PathBuf::from(std::env::var_os(name).expect("required verified artifact input is missing")) }
fn path_hash(path: &Path) -> String {
    let mut file=File::open(path).unwrap();let mut hash=Sha256::new();let mut buf=[0u8;65536];
    loop {let n=file.read(&mut buf).unwrap();if n==0 {break;} hash.update(&buf[..n]);}
    format!("{:x}",hash.finalize())
}
fn read_release(path: &Path) -> ApiRelease { serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap() }
fn native_asset() -> VerifiedAsset {
    select_asset_for_platform(&read_release(&fixture("native-release.json")),"1.0.103",CommunityPlatform::WindowsX86_64Gnu).unwrap()
}
fn native_archive() -> PathBuf {fixture("grok-zh-1.0.103-windows-x86_64-gnu.zip")}
fn genuine_old_copy(dir: &Path) -> PathBuf {
    let old=required_input("GROK_DIRECT_HISTORICAL_EXE");assert_eq!(path_hash(&old),OLD_HASH);
    let destination=dir.join("grok-zh.exe");std::fs::copy(old,&destination).unwrap();destination
}
fn result_file(name: &str, value: serde_json::Value) {
    let dir=required_input("GROK_HISTORICAL_TEST_RESULTS");std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name),serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}
fn file_stream(path: &Path, truncate_stream: bool) -> Result<impl Stream<Item=Result<Vec<u8>>>> {
    let file=File::open(path)?;
    let remaining=if truncate_stream {file.metadata()?.len().saturating_sub(1)} else {u64::MAX};
    Ok(futures::stream::try_unfold((file,remaining), |(mut file,remaining)| async move {
        if remaining==0 {return Ok::<_,anyhow::Error>(None);}
        let mut bytes=vec![0u8;remaining.min(65536) as usize];let n=file.read(&mut bytes)?;
        if n==0 {return Ok(None);} bytes.truncate(n);
        Ok(Some((bytes,(file,remaining-n as u64))))
    }))
}
fn assert_clean_rejection(dir: &Path, destination: &Path) {
    assert_eq!(path_hash(destination),OLD_HASH);
    let leftovers:Vec<_>=std::fs::read_dir(dir).unwrap().map(|e|e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    assert_eq!(leftovers,vec!["grok-zh.exe".to_string()],"rejection left temporary files");
}

#[test]
fn discovery_uses_real_historical_metadata_and_explicit_unpublished_candidate() {
    let actual=read_release(&Path::new(env!("CARGO_MANIFEST_DIR")).join("metadata/historical-release-v1.0.45.json"));
    let (_,baseline)=select_latest_compatible_release(&[actual.clone()],"stable",CommunityPlatform::WindowsX86_64Gnu).map(|(r,v)|(r.clone(),v)).unwrap();
    assert_eq!(baseline.to_string(),"1.0.45");
    let candidate=read_release(&fixture("native-release.json"));
    let mut ineligible=candidate.clone();ineligible.tag_name="release-v9.9.9".to_string();ineligible.draft=true;
    let list=vec![actual,candidate,ineligible];
    let (selected,version)=select_latest_compatible_release(&list,"stable",CommunityPlatform::WindowsX86_64Gnu).unwrap();
    assert_eq!(version.to_string(),"1.0.103");assert_eq!(selected.tag_name,"release-v1.0.103");
    assert!(select_latest_compatible_release(&list,"invalid",CommunityPlatform::WindowsX86_64Gnu).is_err());
    result_file("discovery-evidence.json",json!({"status":"PASSED","real_snapshot":"release-v1.0.45","selected_future_candidate":"release-v1.0.103","future_candidate_is_synthetic_unpublished":true,"covered":"select_latest_compatible_release and select_asset_for_platform","not_covered":["HTTP API fetch and pagination","runtime GNU platform discovery inside genuine old binary"]}));
}

#[tokio::test]
async fn genuine_old_archive_passes_unchanged_stream_hash_extraction_and_probe() {
    let release=read_release(&Path::new(env!("CARGO_MANIFEST_DIR")).join("metadata/historical-release-v1.0.45.json"));
    let asset=select_asset_for_platform(&release,"1.0.45",CommunityPlatform::WindowsX86_64Gnu).unwrap();
    let source=required_input("GROK_DIRECT_HISTORICAL_ARCHIVE");let dir=tempfile::tempdir().unwrap();
    let downloaded=dir.path().join("download.zip");let exe=dir.path().join("grok-zh.exe");
    write_verified_download(&asset,&downloaded,&ProgressBar::hidden(),file_stream(&source,false).unwrap()).await.unwrap();
    crate::community_release::extract_verified_executable(&asset,&downloaded,&exe).unwrap();
    assert_eq!(path_hash(&exe),OLD_HASH);
    let version=smoke_test_binary(&exe).await.unwrap();assert!(community_version_output_matches(&version,"1.0.45"));
    result_file("historical-baseline-evidence.json",json!({"status":"PASSED","exe_sha256":OLD_HASH,"version_output":version,"smoke_timeout_seconds":SMOKE_TEST_TIMEOUT.as_secs(),"metadata":"actual captured immutable GitHub release metadata","transport":"local real archive bytes streamed through original write_verified_download"}));
}

#[tokio::test]
async fn full_isolated_legacy_transaction_upgrades_genuine_old_to_native_and_probes() {
    let dir=tempfile::tempdir().unwrap();let destination=genuine_old_copy(dir.path());
    let old_version=smoke_test_binary(&destination).await.unwrap();assert!(community_version_output_matches(&old_version,"1.0.45"));
    let cancellation=crate::community_update_cancel::Cancellation::new();
    let installed=cancellation.scope(isolated_transaction(native_asset(),destination.clone(),&native_archive(),false)).await.unwrap();
    assert_eq!(installed,"1.0.103");assert_eq!(path_hash(&destination),NEW_HASH);
    let new_version=smoke_test_binary(&destination).await.unwrap();assert!(community_version_output_matches(&new_version,"1.0.103"));
    for entry in std::fs::read_dir(dir.path()).unwrap() {let name=entry.unwrap().file_name().to_string_lossy().into_owned();assert!(!name.ends_with(".candidate.exe")&&!name.ends_with(".download.zip"));}
    result_file("legacy-transaction-evidence.json",json!({"status":"PASSED","old_sha256":OLD_HASH,"native_sha256":NEW_HASH,"old_probe":old_version,"post_install_probe":new_version,"version":"1.0.103","smoke_timeout_seconds":SMOKE_TEST_TIMEOUT.as_secs(),"unchanged_transaction_slices":["auto_update.rs:1173-1174","auto_update.rs:1177-1231"],"seams":["explicit isolated destination containing genuine historical EXE","local real byte stream replacing HTTPS adapter"],"whole_install_community_release_executed":false,"genuine_old_binary_network_update_executed":false,"live_https_adapter_executed":false,"success_receipt_and_user_config_executed":false}));
}

#[tokio::test]
async fn wrong_outer_hash_rejects_real_stream_and_cleans_without_changing_old_exe() {
    let dir=tempfile::tempdir().unwrap();let destination=genuine_old_copy(dir.path());let mut asset=native_asset();asset.sha256="0".repeat(64);
    let error=isolated_transaction(asset,destination.clone(),&native_archive(),false).await.unwrap_err();
    assert!(error.to_string().contains("SHA-256"),"{error:#}");assert_clean_rejection(dir.path(),&destination);
}
#[tokio::test]
async fn truncated_real_stream_rejects_and_cleans_without_changing_old_exe() {
    let dir=tempfile::tempdir().unwrap();let destination=genuine_old_copy(dir.path());
    let error=isolated_transaction(native_asset(),destination.clone(),&native_archive(),true).await.unwrap_err();
    assert!(error.to_string().contains("truncated"),"{error:#}");assert_clean_rejection(dir.path(),&destination);
}
#[tokio::test]
async fn wrong_real_native_version_rejects_at_historical_probe_gate_and_cleans() {
    let dir=tempfile::tempdir().unwrap();let destination=genuine_old_copy(dir.path());
    let release=read_release(&fixture("wrong-version-release.json"));let asset=select_asset_for_platform(&release,"1.0.104",CommunityPlatform::WindowsX86_64Gnu).unwrap();
    let error=isolated_transaction(asset,destination.clone(),&fixture("grok-zh-1.0.104-windows-x86_64-gnu.zip"),false).await.unwrap_err();
    assert!(error.to_string().contains("unexpected version"),"must reach actual binary version gate: {error:#}");assert_clean_rejection(dir.path(),&destination);
}
#[test]
fn original_windows_update_lock_rejects_contention_and_releases() {
    let dir=tempfile::tempdir().unwrap();let destination=genuine_old_copy(dir.path());
    let held=acquire_windows_update_lock(&destination).unwrap();assert!(acquire_windows_update_lock(&destination).is_err());
    drop(held);let reacquired=acquire_windows_update_lock(&destination).unwrap();drop(reacquired);assert_eq!(path_hash(&destination),OLD_HASH);
}
#[tokio::test]
async fn genuine_old_file_sharing_lock_exercises_original_rename_fallback() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir=tempfile::tempdir().unwrap();let destination=genuine_old_copy(dir.path());
    // Genuine historical bytes, with a sharing handle that denies write but
    // permits rename/delete. This is NOT a running old CLI process test.
    let mut held=OpenOptions::new().read(true).share_mode(0x1|0x4).open(&destination).unwrap();
    let denied=OpenOptions::new().write(true).open(&destination).unwrap_err();
    assert!(matches!(denied.raw_os_error(),Some(5)|Some(32)),"write sharing was not denied: {denied}");
    isolated_transaction(native_asset(),destination.clone(),&native_archive(),false).await.unwrap();
    assert_eq!(path_hash(&destination),NEW_HASH);held.rewind().unwrap();let mut hash=Sha256::new();let mut buf=[0u8;65536];
    loop {let n=held.read(&mut buf).unwrap();if n==0 {break;}hash.update(&buf[..n]);}
    assert_eq!(format!("{:x}",hash.finalize()),OLD_HASH);drop(held);
    let output=smoke_test_binary(&destination).await.unwrap();assert!(community_version_output_matches(&output,"1.0.103"));
    result_file("sharing-lock-evidence.json",json!({"status":"PASSED","native_sha256":NEW_HASH,"old_open_handle_sha256":OLD_HASH,"scenario":"share-mode handle denies writing genuine old EXE but permits rename; not a running process","post_install_probe":output}));
}
#[tokio::test]
async fn original_replacement_rolls_back_if_source_copy_stays_sharing_locked() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir=tempfile::tempdir().unwrap();let destination=genuine_old_copy(dir.path());let candidate=dir.path().join("candidate.exe");
    let asset=native_asset();crate::community_release::extract_verified_executable(&asset,&native_archive(),&candidate).unwrap();
    assert_eq!(path_hash(&candidate),NEW_HASH);
    let held=OpenOptions::new().read(true).share_mode(0).open(&candidate).unwrap();
    let lock=acquire_windows_update_lock(&destination).unwrap();let result=windows_replace_exe_locked(&candidate,&destination).await;drop(lock);drop(held);
    assert!(result.is_err());assert_eq!(path_hash(&destination),OLD_HASH);assert_eq!(path_hash(&candidate),NEW_HASH);
    assert!(!dir.path().join("grok-zh.exe.old").exists());
    let output=smoke_test_binary(&destination).await.unwrap();assert!(community_version_output_matches(&output,"1.0.45"));
    result_file("rollback-evidence.json",json!({"status":"PASSED","restored_exe_sha256":OLD_HASH,"old_probe":output,"scenario":"original copy/rename/copy/rollback path; source genuine native EXE held unreadable by another handle"}));
}
