use super::*;
use serde_json::{json, Value};
use tempfile::tempdir;
use zip::write::SimpleFileOptions;

const VERSION: &str = "1.0.103";
const GNU: &str = "x86_64-pc-windows-gnu";
const MSVC: &str = "x86_64-pc-windows-msvc";
const NAME: &str = "grok-zh-1.0.103-windows-x86_64-gnu.zip";
const ROOT: &str = "grok-zh-1.0.103-windows-x86_64-gnu";
const NATIVE_HASH: &str = "8626e60c908f44848190adc9f3547241c651ed8179601d93e33b91469dc76f20";

fn protocol_value() -> Value {
    json!({"schema":1,"version":VERSION,"platform":GNU,"mode":"executable-only",
           "manifest":"SHA256SUMS.txt","executable":"grok-zh.exe","installer":"Install-GrokZh.ps1"})
}
fn block(value: &Value) -> Vec<u8> {
    format!("GROK-UPDATE-PROTOCOL-BEGIN\n{}\nGROK-UPDATE-PROTOCOL-END\n", value).into_bytes()
}
fn parse(value: &Value) -> Result<Option<PackageProtocol>> {
    PackageProtocol::parse(&block(value), VERSION, GNU)
}
fn sum(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }
fn file_sum(path: &Path) -> String {
    let mut reader = File::open(path).unwrap();
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buffer).unwrap();
        if n == 0 { break; }
        hasher.update(&buffer[..n]);
    }
    format!("{:x}", hasher.finalize())
}
fn release() -> ApiRelease {
    serde_json::from_value(json!({
        "tag_name":"release-v1.0.103", "draft":false,"prerelease":false,"immutable":true,
        "assets":[{"name":NAME,"browser_download_url":format!("https://github.com/JoyElliot/grok-build-Chinese/releases/download/release-v1.0.103/{NAME}"),
            "size":123,"digest":format!("sha256:{}", "a".repeat(64)),"state":"uploaded"}]
    })).unwrap()
}
fn selected(r: &ApiRelease) -> Result<VerifiedAsset> {
    select_asset_for_platform(r, VERSION, CommunityPlatform::WindowsX86_64Gnu)
}
fn metadata_hashes(info: &[u8], exe: &[u8], installer: &[u8]) -> String {
    format!("{}  grok-zh.exe\n{}  Install-GrokZh.ps1\n{}  BUILD-INFO.txt\n",sum(exe),sum(installer),sum(info))
}
fn create_fixture(path: &Path, root: &str, info: &[u8], corrupt_exe: bool, extra: bool) {
    let inert = b"INERT PARSER TEST DATA. THIS IS NOT AN EXECUTABLE.";
    let installer = b"# Inert parser fixture, never executed\n";
    let manifest = metadata_hashes(info, inert, installer);
    let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let damaged = b"CHANGED INERT PARSER TEST DATA";
    for (name, bytes) in [
        ("grok-zh.exe", if corrupt_exe { damaged.as_slice() } else { inert.as_slice() }),
        ("Install-GrokZh.ps1", installer.as_slice()),
        ("BUILD-INFO.txt", info),
        ("SHA256SUMS.txt", manifest.as_bytes()),
    ] {
        writer.start_file(format!("{root}/{name}"), opts).unwrap();
        writer.write_all(bytes).unwrap();
    }
    if extra { writer.start_file(format!("{root}/unexpected.txt"), opts).unwrap(); }
    writer.finish().unwrap();
}

#[test]
fn positive_protocol_accepts_truthful_msvc_extension_in_legacy_transport() {
    let info = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"),"/fixtures/native-build-info.txt")).unwrap();
    let text = std::str::from_utf8(&info).unwrap();
    assert!(text.contains("Target: x86_64-pc-windows-msvc"));
    assert!(text.contains("\"actual_runtime_target\": \"x86_64-pc-windows-msvc\""));
    let p = PackageProtocol::parse(&info, VERSION, GNU).unwrap().unwrap();
    assert_eq!(p.executable, "grok-zh.exe");
    assert_eq!(p.installer, "Install-GrokZh.ps1");
    // The extension is intentionally not interpreted by historical serde. This
    // positive test establishes tolerance, not runtime/ABI validation.
}

#[test]
fn negative_protocol_platform_msvc_is_not_accepted_by_legacy_selector() {
    let mut p=protocol_value();p["platform"]=json!(MSVC);
    assert!(parse(&p).is_err());
}
#[test]
fn negative_protocol_version_must_equal_requested_release() {
    let mut p=protocol_value();p["version"]=json!("1.0.104");assert!(parse(&p).is_err());
}
#[test]
fn negative_protocol_schema_mode_manifest_and_types() {
    for (field,value) in [("schema",json!(2)),("schema",json!("1")),("mode",json!("full-install")),("manifest",json!("OTHER.txt"))] {
        let mut p=protocol_value();p[field]=value;assert!(parse(&p).is_err(),"accepted {field}");
    }
    let mut p=protocol_value();p.as_object_mut().unwrap().remove("schema");assert!(parse(&p).is_err());
}
#[test]
fn negative_protocol_entrypoint_names() {
    for path in ["../grok.exe","/grok.exe","grok:exe","CON.exe","docs/COM¹.txt","SHA256SUMS.txt","BUILD-INFO.txt","Install-GrokZh.ps1"] {
        let mut p=protocol_value();p["executable"]=json!(path);assert!(parse(&p).is_err(),"accepted {path}");
    }
}
#[test]
fn negative_protocol_duplicate_or_partial_markers() {
    let mut bytes=block(&protocol_value());bytes.extend(b"GROK-UPDATE-PROTOCOL-BEGIN\n");
    assert!(PackageProtocol::parse(&bytes,VERSION,GNU).is_err());
    assert!(PackageProtocol::parse(b"GROK-UPDATE-PROTOCOL-END\n",VERSION,GNU).is_err());
}
#[test]
fn missing_protocol_returns_legacy_fallback_not_declarative_acceptance() {
    assert!(PackageProtocol::parse(b"legacy build info\n",VERSION,GNU).unwrap().is_none());
}
#[test]
fn negative_protocol_utf8_and_metadata_size() {
    assert!(PackageProtocol::parse(&[0xff],VERSION,GNU).is_err());
    assert!(PackageProtocol::parse(&vec![b'x';MAX_MANIFEST_BYTES as usize+1],VERSION,GNU).is_err());
}
#[test]
fn negative_manifest_hash_format_names_duplicates_and_count() {
    for text in ["z".repeat(64)+"  grok-zh.exe\n", "a".repeat(63)+"  grok-zh.exe\n", "a".repeat(64)+" grok-zh.exe\n", "a".repeat(64)+"  ../bad\n", "a".repeat(64)+"  SHA256SUMS.txt\n", String::new(), format!("{0}  test\n{0}  TEST\n","a".repeat(64)), format!("{0}  file\n{0}  file/child\n","a".repeat(64))] {
        assert!(package_protocol::parse_manifest(text.as_bytes()).is_err(),"accepted {text}");
    }
}
#[test]
fn negative_manifest_must_cover_executable_installer_and_build_info() {
    let p=parse(&protocol_value()).unwrap().unwrap();
    let all=["grok-zh.exe","Install-GrokZh.ps1","BUILD-INFO.txt"];
    for missing in all {
        let mut hashes=HashMap::new();
        for name in all {if name!=missing {hashes.insert(name.to_string(),"a".repeat(64));}}
        assert!(p.check_manifest(&hashes).is_err(),"accepted missing {missing}");
    }
}
#[test]
fn positive_release_metadata_accepts_gnu_transport_name() {
    let asset=selected(&release()).unwrap();assert_eq!(asset.name,NAME);
    assert_eq!(asset.archive_kind,CommunityArchiveKind::WindowsZip);
}
#[test]
fn negative_release_msvc_name_and_wrong_url() {
    let mut r=release();r.assets[0].name="grok-zh-1.0.103-windows-x86_64-msvc.zip".to_string();assert!(selected(&r).is_err());
    let mut r=release();r.assets[0].browser_download_url.push_str("?altered=true");assert!(selected(&r).is_err());
}
#[test]
fn negative_release_version_tag_canonicality() {
    for version in ["v1.0.103","1.0.103+build","01.0.103","1.0"] {assert!(canonical_release_version(version).is_err());}
    let mut r=release();r.tag_name="release-v1.0.104".to_string();assert!(selected(&r).is_err());
    let mut r=release();r.tag_name="v1.0.103".to_string();assert!(selected(&r).is_err());
}
#[test]
fn negative_release_must_be_immutable_nondraft_and_consistent_prerelease() {
    let mut r=release();r.immutable=false;assert!(selected(&r).is_err());
    let mut r=release();r.draft=true;assert!(selected(&r).is_err());
    let mut r=release();r.prerelease=true;assert!(selected(&r).is_err());
}
#[test]
fn negative_release_asset_hash_missing_or_malformed() {
    for digest in [None,Some("sha256:abc".to_string()),Some(format!("sha512:{}","a".repeat(64))),Some(format!("sha256:{}","z".repeat(64)))] {
        let mut r=release();r.assets[0].digest=digest;assert!(selected(&r).is_err());
    }
}
#[test]
fn negative_release_asset_duplicates_state_and_size() {
    let mut r=release();r.assets.push(r.assets[0].clone());assert!(selected(&r).is_err());
    let mut r=release();r.assets[0].state="new".to_string();assert!(selected(&r).is_err());
    for size in [0,MAX_ASSET_BYTES+1] {let mut r=release();r.assets[0].size=size;assert!(selected(&r).is_err());}
}
#[test]
fn limitation_valid_digest_shape_does_not_authenticate_payload_bytes() {
    let mut r=release();r.assets[0].digest=Some(format!("sha256:{}","0".repeat(64)));
    assert!(selected(&r).is_ok()); // Actual download_verified must compare bytes.
}
#[test]
fn bridge_v108_name_policy_remains_two_assets_and_old_namespace() {
    assert_eq!(release_tag_for_version(&Version::parse("1.0.8").unwrap()),"v1.0.8");
    assert_eq!(expected_release_asset_names("1.0.8").unwrap(),vec!["grok-zh-1.0.8-windows-x86_64-gnu.zip".to_string(),"grok-zh-1.0.8-windows-x86_64-gnu.zip.sha256".to_string()]);
    assert_eq!(expected_release_asset_names("1.0.45").unwrap().len(),6);
    // This helper encodes historical expectations. It does not execute the
    // published v1.0.5 binary or validate the published v1.0.8 archive bytes.
}
#[test]
fn positive_archive_extracts_only_declared_synthetic_file() {
    let dir=tempdir().unwrap();let zip=dir.path().join("fixture.zip");let out=dir.path().join("candidate.exe");
    create_fixture(&zip,ROOT,&block(&protocol_value()),false,false);
    extract_verified_windows_executable(&selected(&release()).unwrap(),&zip,&out).unwrap();
    assert_eq!(std::fs::read(out).unwrap(),b"INERT PARSER TEST DATA. THIS IS NOT AN EXECUTABLE.");
    assert!(!dir.path().join("Install-GrokZh.ps1").exists());
}
#[test]
fn negative_archive_hash_mismatch_rejected_and_candidate_removed() {
    let dir=tempdir().unwrap();let zip=dir.path().join("fixture.zip");let out=dir.path().join("candidate.exe");
    create_fixture(&zip,ROOT,&block(&protocol_value()),true,false);
    let err=extract_verified_windows_executable(&selected(&release()).unwrap(),&zip,&out).unwrap_err();
    assert!(err.to_string().contains("SHA-256 mismatch"),"{err:#}");assert!(!out.exists());
}
#[test]
fn negative_archive_root_extra_file_and_asset_name() {
    for (root,extra) in [("wrong-root",false),(ROOT,true)] {
        let dir=tempdir().unwrap();let zip=dir.path().join("fixture.zip");let out=dir.path().join("candidate.exe");
        create_fixture(&zip,root,&block(&protocol_value()),false,extra);
        assert!(extract_verified_windows_executable(&selected(&release()).unwrap(),&zip,&out).is_err());assert!(!out.exists());
    }
    let dir=tempdir().unwrap();let zip=dir.path().join("fixture.zip");let out=dir.path().join("candidate.exe");
    create_fixture(&zip,ROOT,&block(&protocol_value()),false,false);
    let mut asset=selected(&release()).unwrap();asset.name=asset.name.replace("-gnu.zip","-msvc.zip");
    assert!(extract_verified_windows_executable(&asset,&zip,&out).is_err());assert!(!out.exists());
}
#[test]
fn negative_existing_extraction_destination_is_never_overwritten() {
    let dir=tempdir().unwrap();let zip=dir.path().join("fixture.zip");let out=dir.path().join("candidate.exe");
    create_fixture(&zip,ROOT,&block(&protocol_value()),false,false);std::fs::write(&out,b"keep").unwrap();
    assert!(extract_verified_windows_executable(&selected(&release()).unwrap(),&zip,&out).is_err());assert_eq!(std::fs::read(out).unwrap(),b"keep");
}
#[test]
fn native_msvc_payload_full_package_extracts_byte_identically() {
    let fixtures=Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let r:ApiRelease=serde_json::from_slice(&std::fs::read(fixtures.join("native-release.json")).unwrap()).unwrap();
    let asset=selected(&r).unwrap();let zip=fixtures.join(NAME);
    // Test harness authentication of local fixture, not a live network call.
    assert_eq!(std::fs::metadata(&zip).unwrap().len(),asset.size);
    assert_eq!(file_sum(&zip),asset.sha256);
    let dir=tempdir().unwrap();let out=dir.path().join("isolated-native-candidate.exe");
    extract_verified_windows_executable(&asset,&zip,&out).unwrap();
    assert_eq!(file_sum(&out),NATIVE_HASH);
    assert_eq!(std::fs::metadata(&out).unwrap().len(),141354496);
    // The EXE is never executed or activated. A PE header cannot establish
    // compiler provenance; CI/source/BUILD-INFO evidence supplies that claim.
}
#[cfg(all(target_os="windows",target_arch="x86_64",target_env="msvc"))]
#[test]
fn historical_current_platform_rejects_native_msvc_build() {
    assert!(current_community_platform().is_err());
}
