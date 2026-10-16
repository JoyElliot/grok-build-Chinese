//! Harness-only fixtures. Every extraction/verification function under test is unchanged source.
use super::*;
use serde_json::json;
use tempfile::tempdir;
use zip::write::SimpleFileOptions;

const VERSION: &str = "1.0.103";
const GNU: &str = "x86_64-pc-windows-gnu";
const MSVC: &str = "x86_64-pc-windows-msvc";
const EXPECTED_NATIVE_EXE_SHA256: &str = "8626e60c908f44848190adc9f3547241c651ed8179601d93e33b91469dc76f20";
const REJECTION: &str = "GNU release is not a recognized embedded MSVC package; use its full installer for a GNU downgrade";

fn sum_reader(mut reader: impl Read) -> String {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let size = reader.read(&mut buffer).unwrap();
        if size == 0 { break; }
        hasher.update(&buffer[..size]);
    }
    format!("{:x}", hasher.finalize())
}

fn synthetic_release(name: &str, path: &Path) -> ApiRelease {
    serde_json::from_value(json!({
        "tag_name":format!("release-v{VERSION}"),
        "body":"SYNTHETIC OFFLINE METADATA; never published or fetched",
        "draft":false,"prerelease":false,"immutable":true,
        "assets":[{
            "name":name,
            "browser_download_url":format!("https://github.com/JoyElliot/grok-build-Chinese/releases/download/release-v{VERSION}/{name}"),
            "state":"uploaded","size":std::fs::metadata(path).unwrap().len(),
            "digest":format!("sha256:{}",sum_reader(File::open(path).unwrap()))
        }]
    })).unwrap()
}

fn make_inert_fixture(path: &Path, platform: CommunityPlatform) -> Vec<u8> {
    let name = release_asset_name_for(platform, VERSION).unwrap();
    let root = name.strip_suffix(".zip").unwrap();
    let data = b"INERT NON-EXECUTABLE TEST BYTES. No process is started.".to_vec();
    let installer = b"# Inert fixture. Never executed.\n";
    let protocol = json!({
        "schema":1,"version":VERSION,"platform":platform.target_triple(),
        "actual_runtime_target":MSVC,"mode":"executable-only","manifest":"SHA256SUMS.txt",
        "executable":"grok-zh.exe","installer":"Install-GrokZh.ps1"
    });
    let info = format!("Target: {MSVC}\nGROK-UPDATE-PROTOCOL-BEGIN\n{protocol}\nGROK-UPDATE-PROTOCOL-END\n");
    let manifest = format!("{}  grok-zh.exe\n{}  Install-GrokZh.ps1\n{}  BUILD-INFO.txt\n",
        sum_reader(data.as_slice()), sum_reader(installer.as_slice()), sum_reader(info.as_bytes()));
    let mut archive = zip::ZipWriter::new(File::create(path).unwrap());
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (entry, bytes) in [
        ("grok-zh.exe", data.as_slice()),
        ("Install-GrokZh.ps1", installer.as_slice()),
        ("BUILD-INFO.txt", info.as_bytes()),
        ("SHA256SUMS.txt", manifest.as_bytes()),
    ] {
        archive.start_file(format!("{root}/{entry}"), options).unwrap();
        archive.write_all(bytes).unwrap();
    }
    archive.finish().unwrap();
    data
}

#[test]
fn direct_gnu_transport_without_embedded_runtime_is_rejected() {
    let temporary = tempdir().unwrap();
    let path = temporary.path().join("direct-gnu.zip");
    make_inert_fixture(&path, CommunityPlatform::WindowsX86_64Gnu);
    let name = release_asset_name_for(CommunityPlatform::WindowsX86_64Gnu, VERSION).unwrap();
    let release = synthetic_release(&name, &path);
    let selected = select_asset_for_platform(&release, VERSION, CommunityPlatform::WindowsX86_64Msvc).unwrap();
    assert_eq!(selected.name, name);
    assert_eq!(selected.target_platform, CommunityPlatform::WindowsX86_64Msvc);
    let output = temporary.path().join("candidate.exe");
    let error = extract_verified_windows_executable(&selected, &path, &output).unwrap_err();
    assert_eq!(error.to_string(), REJECTION);
    assert!(!output.exists());
    assert!(!output.with_extension("embedded-launcher.tmp").exists());
}

#[test]
fn native_named_package_extracts_declared_inert_bytes() {
    let temporary = tempdir().unwrap();
    let path = temporary.path().join("native.zip");
    let expected = make_inert_fixture(&path, CommunityPlatform::WindowsX86_64Msvc);
    let name = release_asset_name_for(CommunityPlatform::WindowsX86_64Msvc, VERSION).unwrap();
    let release = synthetic_release(&name, &path);
    let selected = select_asset_for_platform(&release, VERSION, CommunityPlatform::WindowsX86_64Msvc).unwrap();
    let output = temporary.path().join("candidate.exe");
    extract_verified_windows_executable(&selected, &path, &output).unwrap();
    assert_eq!(std::fs::read(&output).unwrap(), expected);
    // This tests exact parser extraction, not whether these inert bytes are a valid PE or can execute.
}

#[test]
fn real_native_gnu_transport_is_rejected_for_current_msvc() {
    let fixture_dir = std::env::var_os("GROK_DIRECT_NATIVE_FIXTURE_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures"));
    let metadata = fixture_dir.join("native-release.json");
    let release: ApiRelease = serde_json::from_slice(
        &std::fs::read(&metadata).unwrap_or_else(|error| panic!(
            "Required real-native fixture is missing at {}: {error}. Run prepare_native_fixture.py in the historical parser harness; no test is silently skipped.", metadata.display()))
    ).unwrap();
    let name = release_asset_name_for(CommunityPlatform::WindowsX86_64Gnu, VERSION).unwrap();
    let path = fixture_dir.join(&name);
    let selected = select_asset_for_platform(&release, VERSION, CommunityPlatform::WindowsX86_64Msvc).unwrap();
    assert_eq!(selected.name, name);
    assert_eq!(selected.target_platform, CommunityPlatform::WindowsX86_64Msvc);
    assert_eq!(selected.size, std::fs::metadata(&path).unwrap().len());
    assert_eq!(selected.sha256, sum_reader(File::open(&path).unwrap()));
    {
        let mut archive = zip::ZipArchive::new(File::open(&path).unwrap()).unwrap();
        let root = name.strip_suffix(".zip").unwrap();
        let info = package_protocol::read_zip_metadata(&mut archive, Some(root), BUILD_INFO).unwrap();
        let text = std::str::from_utf8(&info).unwrap();
        assert!(text.contains("Target: x86_64-pc-windows-msvc"));
        assert!(text.contains("\"actual_runtime_target\": \"x86_64-pc-windows-msvc\""));
        let protocol = PackageProtocol::parse(&info, VERSION, GNU).unwrap().unwrap();
        assert!(protocol.embedded_runtime.is_none());
        let exe = archive.by_name(&format!("{root}/grok-zh.exe")).unwrap();
        assert_eq!(sum_reader(exe), EXPECTED_NATIVE_EXE_SHA256);
    }
    let temporary = tempdir().unwrap();
    let output = temporary.path().join("candidate.exe");
    let error = extract_verified_windows_executable(&selected, &path, &output).unwrap_err();
    assert_eq!(error.to_string(), REJECTION);
    assert!(!output.exists());
    assert!(!output.with_extension("embedded-launcher.tmp").exists());
}

#[test]
fn authentic_native_msvc_archive_extracts_identical_native_exe() {
    const EXPECTED_ARCHIVE_SHA256: &str = "802fbd20fee858fc90521438c23d5d9934370c5a97815680c208305e19a2b41d";
    let path = std::env::var_os("GROK_DIRECT_NATIVE_ARCHIVE")
        .map(std::path::PathBuf::from)
        .expect("GROK_DIRECT_NATIVE_ARCHIVE must point to the original pinned grok-zh-1.0.103-windows-x86_64-msvc.zip; the real positive control is never silently skipped");
    assert_eq!(sum_reader(File::open(&path).unwrap()), EXPECTED_ARCHIVE_SHA256,
        "Native archive differs from the unchanged pinned original artifact");
    let name = release_asset_name_for(CommunityPlatform::WindowsX86_64Msvc, VERSION).unwrap();
    let releases = vec![synthetic_release(&name, &path)];
    let (release, version) = select_latest_compatible_release(
        &releases, "stable", CommunityPlatform::WindowsX86_64Msvc,
    ).unwrap();
    assert_eq!(version.to_string(), VERSION);
    let selected = select_asset_for_platform(release, VERSION, CommunityPlatform::WindowsX86_64Msvc).unwrap();
    assert_eq!(selected.name, name);
    assert_eq!(selected.sha256, EXPECTED_ARCHIVE_SHA256);
    assert_eq!(selected.target_platform, CommunityPlatform::WindowsX86_64Msvc);
    let temporary = tempdir().unwrap();
    let output = temporary.path().join("candidate.exe");
    extract_verified_windows_executable(&selected, &path, &output).unwrap();
    assert_eq!(sum_reader(File::open(&output).unwrap()), EXPECTED_NATIVE_EXE_SHA256);
    assert!(!output.with_extension("embedded-launcher.tmp").exists());
    // Exact source extraction succeeds on the authentic native-named archive with
    // the same EXE rejected above in direct GNU transport. No EXE is executed;
    // this is not a second-version update, live download, or activation test.
}
