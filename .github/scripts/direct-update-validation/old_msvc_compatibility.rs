use super::*;

fn write_fixture(path: &Path, version: &str, native: bool) -> Vec<u8> {
    let mut image = vec![0u8; 512];
    image[..2].copy_from_slice(b"MZ");
    image[60..64].copy_from_slice(&128u32.to_le_bytes());
    image[128..134].copy_from_slice(b"PE\0\0\x64\x86");
    image[134..136].copy_from_slice(&1u16.to_le_bytes());
    image[148..150].copy_from_slice(&240u16.to_le_bytes());
    image[152..154].copy_from_slice(&0x20bu16.to_le_bytes());
    let platform = if native { "msvc" } else { "gnu" };
    let mut protocol = serde_json::json!({
        "schema":1, "version":version, "platform":format!("x86_64-pc-windows-{platform}"),
        "mode":"executable-only", "manifest":INNER_MANIFEST,
        "executable":"grok-zh.exe", "installer":"Install-GrokZh.ps1"
    });
    if !native {
        protocol["direct_runtime"] =
            serde_json::json!({"schema":1,"version":version,"platform":"x86_64-pc-windows-msvc"});
    }
    let mut entries: Vec<_> = WINDOWS_REQUIRED_PACKAGE_FILES
        .iter()
        .map(|name| {
            let bytes = match *name {
                "grok-zh.exe" => image.clone(),
                "BUILD-INFO.txt" => {
                    format!("GROK-UPDATE-PROTOCOL-BEGIN\n{protocol}\nGROK-UPDATE-PROTOCOL-END\n")
                        .into_bytes()
                }
                _ => name.as_bytes().to_vec(),
            };
            (name.to_string(), bytes)
        })
        .collect();
    let manifest = entries
        .iter()
        .map(|(name, bytes)| format!("{:x}  {name}\n", Sha256::digest(bytes)))
        .collect::<String>();
    entries.push((INNER_MANIFEST.to_string(), manifest.into_bytes()));
    let mut zip = zip::ZipWriter::new(File::create(path).unwrap());
    for (name, bytes) in entries {
        zip.start_file(
            format!("grok-zh-{version}-windows-x86_64-{platform}/{name}"),
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(&bytes).unwrap();
    }
    zip.finish().unwrap();
    image
}

#[test]
fn old_msvc_prefers_native_in_dual_distribution_and_extracts_direct_native_image() {
    let version = "1.0.110";
    let release = super::direct_compatibility::fixture_release(
        version,
        &["windows-x86_64-gnu.zip", "windows-x86_64-msvc.zip"],
    );
    let (_, selected) = select_latest_compatible_release(
        std::slice::from_ref(&release),
        "stable",
        CommunityPlatform::WindowsX86_64Msvc,
    )
    .unwrap();
    assert_eq!(selected.to_string(), version);
    let asset =
        select_asset_for_platform(&release, version, CommunityPlatform::WindowsX86_64Msvc).unwrap();
    assert_eq!(
        asset.name,
        format!("grok-zh-{version}-windows-x86_64-msvc.zip")
    );
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("native.zip");
    let image = write_fixture(&archive, version, true);
    let output = temp.path().join("candidate.exe");
    extract_verified_executable(&asset, &archive, &output).unwrap();
    assert_eq!(std::fs::read(output).unwrap(), image);
}

#[test]
fn old_msvc_rejects_gnu_direct_only_instead_of_installing_unrecognized_payload() {
    let version = "1.0.110";
    let release =
        super::direct_compatibility::fixture_release(version, &["windows-x86_64-gnu.zip"]);
    let asset =
        select_asset_for_platform(&release, version, CommunityPlatform::WindowsX86_64Msvc).unwrap();
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("direct.zip");
    write_fixture(&archive, version, false);
    let output = temp.path().join("candidate.exe");
    let error = extract_verified_executable(&asset, &archive, &output).unwrap_err();
    assert!(format!("{error:#}").contains("embedded MSVC"));
    assert!(!output.exists());
}
