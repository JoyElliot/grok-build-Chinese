// Harness-only fixtures/assertions. Product functions above remain verbatim.
use serde_json::json;
use std::path::PathBuf;
use std::collections::BTreeMap;

const OLD_HASH: &str = "474918134a312ab5b2200e03092c5f34a99b5f2d3847057fe827e3ff193404ca";
const NEW_HASH: &str = "d7e2e35c9a18ec7d9415ce155cb1677d040925f208aff53f5d29fb7a8ed1d1ce";
const DIRECT_HASH: &str = "ff7ee4339f578b290d2f7ec42236f75889701ae0badc92d462910cb962c10433";
const NATIVE_HASH: &str = "de9e0ea609a8494ccde3024b0b9b6a669f12387d5a92b39e0d73e34d5ab770ff";
const VERSION: &str = "1.0.104";
fn required_input(name: &str) -> PathBuf {
    std::env::var_os(name).map(PathBuf::from).expect("required isolated artifact input is missing")
}
fn path_hash(path: &Path) -> String {
    let mut file = File::open(path).unwrap(); let mut hash = Sha256::new(); let mut buf = [0u8; 65536];
    loop { let n = file.read(&mut buf).unwrap(); if n == 0 { break; } hash.update(&buf[..n]); }
    format!("{:x}", hash.finalize())
}
fn input_archive(direct: bool) -> PathBuf {
    let path = required_input(if direct { "GROK_DIRECT_DIRECT_ARCHIVE" } else { "GROK_DIRECT_NATIVE_ARCHIVE" });
    assert_eq!(path_hash(&path), if direct { DIRECT_HASH } else { NATIVE_HASH }, "unverified product ZIP"); path
}
fn fixture_asset(direct: bool) -> VerifiedAsset {
    let source = input_archive(direct);
    let selected_name = format!("grok-zh-{VERSION}-windows-x86_64-{}.zip", if direct { "gnu" } else { "msvc" });
    // Explicit unpublished metadata fixture, NOT a fetched GitHub Release.
    let mut names = expected_release_asset_names(VERSION).unwrap();
    if !names.contains(&selected_name) { names.push(selected_name.clone()); }
    if direct { names.retain(|name| !name.contains("windows-x86_64-msvc")); }
    let assets: Vec<_> = names.into_iter().map(|name| json!({
        "browser_download_url": format!("https://github.com/{}/releases/download/release-v{VERSION}/{name}", release_repo()),
        "state":"uploaded", "size": if name == selected_name { source.metadata().unwrap().len() } else { 100 },
        "digest":format!("sha256:{}", if name == selected_name { path_hash(&source) } else { "a".repeat(64) }), "name":name
    })).collect();
    let release: ApiRelease = serde_json::from_value(json!({"tag_name":format!("release-v{VERSION}"),"draft":false,"prerelease":false,"immutable":true,"assets":assets})).unwrap();
    select_asset_for_platform(&release, VERSION, fixture_platform()).unwrap()
}
fn snapshot(dir: &Path) -> BTreeMap<String, String> {
    fn recurse(root: &Path, dir: &Path, out: &mut BTreeMap<String,String>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap(); let ty = entry.file_type().unwrap(); assert!(!ty.is_symlink(), "fixture must not contain symlinks");
            if ty.is_dir() { recurse(root,&entry.path(),out); }
            else { assert!(ty.is_file()); out.insert(entry.path().strip_prefix(root).unwrap().to_string_lossy().replace('\\',"/"),path_hash(&entry.path())); }
        }
    }
    let mut out = BTreeMap::new(); recurse(dir,dir,&mut out); out
}
fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap(); let ty = entry.file_type().unwrap(); assert!(!ty.is_symlink());
        let target = destination.join(entry.file_name());
        if ty.is_dir() { copy_tree(&entry.path(),&target); } else { assert!(ty.is_file()); std::fs::copy(entry.path(),target).unwrap(); }
    }
}
fn genuine_copy(dir: &Path) -> (PathBuf, BTreeMap<String,String>) {
    let input = required_input(if LEGACY { "GROK_DIRECT_HISTORICAL_EXE" } else { "GROK_DIRECT_NATIVE_EXE" });
    assert_eq!(path_hash(&input), baseline_hash());
    let package = dir.join("installation"); copy_tree(input.parent().unwrap(),&package);
    let destination = package.join("grok-zh.exe"); assert_eq!(path_hash(&destination), baseline_hash());
    let before = snapshot(&package); (destination,before)
}
fn baseline_hash() -> &'static str { if LEGACY { OLD_HASH } else { NEW_HASH } }
fn baseline_version() -> &'static str { if LEGACY { "1.0.45" } else { VERSION } }
fn unchanged_companions(destination: &Path, before: &BTreeMap<String,String>) {
    let after = snapshot(destination.parent().unwrap());
    for (name,hash) in before { if name != "grok-zh.exe" { assert_eq!(after.get(name),Some(hash),"changed package companion {name}"); } }
    for name in after.keys() {
        if !before.contains_key(name) {
            assert!(name == "grok-zh.exe.update.lock" || name == "grok-zh.exe.old" || (name.starts_with("grok-zh.exe.old.") && name.ends_with(".old")),"unexpected package leftover {name}");
        }
    }
}
fn result_file(case: &str, value: serde_json::Value) {
    let dir = required_input("GROK_TRANSACTION_RESULTS"); std::fs::create_dir_all(&dir).unwrap();
    let mode = if LEGACY { "legacy" } else { "current" };
    std::fs::write(dir.join(format!("{mode}-{case}.json")),serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}
fn file_stream(path: &Path, truncate: bool) -> Result<impl Stream<Item=Result<Vec<u8>>>> {
    let file = File::open(path)?; let remaining = if truncate { file.metadata()?.len().saturating_sub(1) } else { u64::MAX };
    Ok(futures::stream::try_unfold((file,remaining), |(mut file,remaining)| async move {
        if remaining == 0 { return Ok::<_,anyhow::Error>(None); }
        let mut bytes = vec![0u8;remaining.min(65536) as usize]; let n = file.read(&mut bytes)?;
        if n == 0 { return Ok(None); } bytes.truncate(n); Ok(Some((bytes,(file,remaining-n as u64))))
    }))
}
fn assert_rejection_unchanged(destination: &Path,before: &BTreeMap<String,String>) {
    assert_eq!(path_hash(destination),baseline_hash()); unchanged_companions(destination,before);
    for entry in std::fs::read_dir(destination.parent().unwrap()).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        assert!(!name.ends_with(".candidate.exe") && !name.ends_with(".download.zip") && !name.ends_with(".staged.exe"),"temporary file leaked: {name}");
    }
}
#[tokio::test]
async fn full_transaction_uses_real_package_and_original_activation() {
    let dir = tempfile::tempdir().unwrap(); let (destination,before) = genuine_copy(dir.path());
    let old_probe = smoke_test_binary(&destination).await.unwrap(); assert!(community_version_output_matches(&old_probe,baseline_version()));
    let cancel = crate::community_update_cancel::Cancellation::new();
    let installed = cancel.scope(isolated_transaction(fixture_asset(LEGACY),destination.clone(),&input_archive(LEGACY),false)).await.unwrap();
    assert_eq!(installed,VERSION); assert_eq!(path_hash(&destination),NEW_HASH); unchanged_companions(&destination,&before);
    let probe = smoke_test_binary(&destination).await.unwrap(); assert!(community_version_output_matches(&probe,VERSION));
    result_file("transaction",json!({"status":"PASSED","initial_sha256":baseline_hash(),"installed_sha256":NEW_HASH,"initial_version":baseline_version(),"installed_version":VERSION,"initial_probe":old_probe,"installed_probe":probe,"scenario":if LEGACY {"historical GNU 1.0.45 source transaction installs genuine MSVC 1.0.104"} else {"current MSVC source same-version explicit reinstall of genuine 1.0.104"},"whole_immutable_old_executable_update":false,"live_https":false,"transport":"fixed authentic ZIP bytes via original stream validator; synthetic unpublished metadata","destination":"copy of full original package; companions unchanged"}));
}
#[tokio::test]
async fn wrong_outer_hash_rejects_and_preserves_real_initial_package() {
    let dir=tempfile::tempdir().unwrap();let (destination,before)=genuine_copy(dir.path());let mut asset=fixture_asset(LEGACY);asset.sha256="0".repeat(64);
    let error=isolated_transaction(asset,destination.clone(),&input_archive(LEGACY),false).await.unwrap_err();
    assert!(format!("{error:#}").contains("SHA-256"));assert_rejection_unchanged(&destination,&before);
    result_file("hash-rejection",json!({"status":"PASSED","preserved_sha256":baseline_hash()}));
}
#[tokio::test]
async fn truncated_stream_rejects_and_preserves_real_initial_package() {
    let dir=tempfile::tempdir().unwrap();let (destination,before)=genuine_copy(dir.path());
    let error=isolated_transaction(fixture_asset(LEGACY),destination.clone(),&input_archive(LEGACY),true).await.unwrap_err();
    assert!(format!("{error:#}").contains("truncated"));assert_rejection_unchanged(&destination,&before);
    result_file("truncation-rejection",json!({"status":"PASSED","preserved_sha256":baseline_hash()}));
}
#[test]
fn original_update_lock_contention_and_release() {
    let dir=tempfile::tempdir().unwrap();let (destination,before)=genuine_copy(dir.path());
    let held=acquire_windows_update_lock(&destination).unwrap();assert!(acquire_windows_update_lock(&destination).is_err());drop(held);
    let held=acquire_windows_update_lock(&destination).unwrap();drop(held);assert_rejection_unchanged(&destination,&before);
    result_file("lock-contention",json!({"status":"PASSED"}));
}
#[tokio::test]
async fn sharing_locked_real_initial_image_preserves_open_handle() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir=tempfile::tempdir().unwrap();let (destination,before)=genuine_copy(dir.path());
    let mut held=OpenOptions::new().read(true).share_mode(0x1|0x4).open(&destination).unwrap();
    assert!(matches!(OpenOptions::new().write(true).open(&destination).unwrap_err().raw_os_error(),Some(5)|Some(32)));
    isolated_transaction(fixture_asset(LEGACY),destination.clone(),&input_archive(LEGACY),false).await.unwrap();
    assert_eq!(path_hash(&destination),NEW_HASH);let mut hash=Sha256::new();let mut buf=[0u8;65536];
    loop {let n=held.read(&mut buf).unwrap();if n==0 {break;}hash.update(&buf[..n]);}
    assert_eq!(format!("{:x}",hash.finalize()),baseline_hash());drop(held);unchanged_companions(&destination,&before);
    assert!(community_version_output_matches(&smoke_test_binary(&destination).await.unwrap(),VERSION));
    result_file("sharing-lock",json!({"status":"PASSED","open_handle_sha256":baseline_hash(),"installed_sha256":NEW_HASH,"not_a_running_old_process":true}));
}
#[tokio::test]
async fn source_sharing_failure_preserves_real_initial_image() {
    use std::os::windows::fs::OpenOptionsExt;
    let dir=tempfile::tempdir().unwrap();let (destination,before)=genuine_copy(dir.path());let candidate=dir.path().join("candidate.exe");
    extract_verified_executable(&fixture_asset(LEGACY),&input_archive(LEGACY),&candidate).unwrap();assert_eq!(path_hash(&candidate),NEW_HASH);
    let held=OpenOptions::new().read(true).share_mode(0).open(&candidate).unwrap();
    let lock=acquire_windows_update_lock(&destination).unwrap();let result=windows_replace_exe_locked(&candidate,&destination).await;drop(lock);drop(held);
    assert!(result.is_err());assert_rejection_unchanged(&destination,&before);assert_eq!(path_hash(&candidate),NEW_HASH);
    assert!(community_version_output_matches(&smoke_test_binary(&destination).await.unwrap(),baseline_version()));
    result_file("source-sharing-failure",json!({"status":"PASSED","preserved_sha256":baseline_hash(),"scenario":if LEGACY {"original copy/rename/copy/rollback failure path"} else {"new updater source staging failure before touching installed EXE"}}));
}

#[tokio::test]
async fn mismatched_declared_version_reaches_real_candidate_probe_and_rejects() {
    // Negative fixture ONLY: change archive metadata to 1.0.105, never EXE bytes.
    // It must fail; this is not a second product build or a successful upgrade.
    let dir=tempfile::tempdir().unwrap();let (destination,before)=genuine_copy(dir.path());
    let source=input_archive(LEGACY);let mut input=zip::ZipArchive::new(File::open(source).unwrap()).unwrap();
    let mut entries=Vec::new();let mut manifest_name=String::new();let mut manifest=String::new();
    for index in 0..input.len() {
        let mut entry=input.by_index(index).unwrap();
        if entry.is_dir() {continue;}
        let name=entry.name().replace(VERSION,"1.0.105");let mut bytes=Vec::new();entry.read_to_end(&mut bytes).unwrap();
        if name.ends_with("/SHA256SUMS.txt") {manifest_name=name;manifest=String::from_utf8(bytes).unwrap();continue;}
        if name.ends_with("/BUILD-INFO.txt") {bytes=String::from_utf8(bytes).unwrap().replace(VERSION,"1.0.105").into_bytes();}
        entries.push((name,bytes));
    }
    let mut hashes=BTreeMap::new();
    for (name,bytes) in &entries {hashes.insert(name.split_once('/').unwrap().1.to_string(),format!("{:x}",Sha256::digest(bytes)));}
    let corrected=manifest.lines().map(|line| {
        let path=line.get(66..).expect("standard fixed product manifest line");
        format!("{}  {path}\n",hashes.get(path).expect("manifest member in fixed source package"))
    }).collect::<String>();
    entries.push((manifest_name,corrected.into_bytes()));
    let archive=dir.path().join("negative-wrong-version.zip");let mut writer=zip::ZipWriter::new(File::create(&archive).unwrap());
    for (name,bytes) in entries {writer.start_file(name,zip::write::SimpleFileOptions::default()).unwrap();writer.write_all(&bytes).unwrap();}writer.finish().unwrap();
    let mut asset=fixture_asset(LEGACY);asset.version="1.0.105".to_string();asset.name=asset.name.replace(VERSION,"1.0.105");asset.sha256=path_hash(&archive);asset.size=archive.metadata().unwrap().len();
    let error=isolated_transaction(asset,destination.clone(),&archive,false).await.unwrap_err();
    assert!(format!("{error:#}").contains("unexpected version"),"must reach actual EXE probe, not fail earlier: {error:#}");
    assert_rejection_unchanged(&destination,&before);
    result_file("real-version-rejection",json!({"status":"PASSED","declared_fixture_version":"1.0.105","unchanged_real_exe_version":VERSION,"preserved_sha256":baseline_hash(),"not_a_successful_future_update":true}));
}
