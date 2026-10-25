use super::*;
#[test]
fn direct_payload_preserves_legacy_transport_and_exact_hash_validation() {
    for version in ["1.0.110", "1.0.111", "1.0.112"] {
        let temp = tempfile::tempdir().unwrap();
        let names = expected_release_asset_names(version).unwrap();
        let assets: Vec<_> = names.into_iter().map(|name| serde_json::json!({
            "browser_download_url": format!("https://github.com/{}/releases/download/release-v{version}/{name}", release_repo()),
            "name": name, "state": "uploaded", "size": 100,
            "digest": format!("sha256:{}", "a".repeat(64))
        })).collect();
        let release: ApiRelease = serde_json::from_value(serde_json::json!({
            "tag_name": format!("release-v{version}"), "draft": false,
            "prerelease": false, "immutable": true, "assets": assets
        }))
        .unwrap();
        let asset =
            select_asset_for_platform(&release, version, CommunityPlatform::WindowsX86_64Gnu)
                .unwrap();
        let root = asset.name.strip_suffix(".zip").unwrap();
        let protocol = serde_json::json!({
            "schema": 1, "version": version, "platform": "x86_64-pc-windows-gnu",
            "mode": "executable-only", "manifest": INNER_MANIFEST,
            "executable": "grok-zh.exe", "installer": "Install-GrokZh.ps1",
            "direct_runtime": {"schema": 1, "version": version, "platform": "x86_64-pc-windows-msvc"}
        });
        let mut image = vec![0u8; 512];
        image[..2].copy_from_slice(b"MZ");
        image[60..64].copy_from_slice(&128u32.to_le_bytes());
        image[128..134].copy_from_slice(b"PE\0\0\x64\x86");
        image[134..136].copy_from_slice(&1u16.to_le_bytes());
        image[148..150].copy_from_slice(&240u16.to_le_bytes());
        image[152..154].copy_from_slice(&0x20bu16.to_le_bytes());
        let mut entries: Vec<_> = WINDOWS_REQUIRED_PACKAGE_FILES
            .iter()
            .map(|name| {
                let data = match *name {
                    "grok-zh.exe" => image.clone(),
                    "BUILD-INFO.txt" => format!(
                        "GROK-UPDATE-PROTOCOL-BEGIN\n{protocol}\nGROK-UPDATE-PROTOCOL-END\n"
                    )
                    .into_bytes(),
                    _ => name.as_bytes().to_vec(),
                };
                (name.to_string(), data)
            })
            .collect();
        let manifest = entries
            .iter()
            .map(|(name, data)| format!("{:x}  {name}\n", Sha256::digest(data)))
            .collect::<String>();
        entries.push((INNER_MANIFEST.to_string(), manifest.into_bytes()));
        let zip_path = temp.path().join("direct.zip");
        let mut writer = zip::ZipWriter::new(File::create(&zip_path).unwrap());
        for (name, data) in entries {
            writer
                .start_file(
                    format!("{root}/{name}"),
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            writer.write_all(&data).unwrap();
        }
        writer.finish().unwrap();
        let output = temp.path().join("candidate.exe");
        extract_verified_executable(&asset, &zip_path, &output).unwrap();
        assert_eq!(std::fs::read(&output).unwrap(), image);
    }
}

pub(super) fn fixture_release(version: &str, suffixes: &[&str]) -> ApiRelease {
    let assets: Vec<_> = suffixes.iter().map(|suffix| {
        let name = format!("grok-zh-{version}-{suffix}");
        serde_json::json!({
            "browser_download_url": format!("https://github.com/{}/releases/download/release-v{version}/{name}", release_repo()),
            "name": name, "state": "uploaded", "size": 100,
            "digest": format!("sha256:{}", "a".repeat(64))
        })
    }).collect();
    serde_json::from_value(serde_json::json!({
        "tag_name": format!("release-v{version}"), "draft": false,
        "prerelease": false, "immutable": true, "assets": assets
    }))
    .unwrap()
}

#[test]
fn dual_distribution_keeps_oldest_bridge_and_modern_gnu_upgrade_route() {
    let bridge = fixture_release(
        "1.0.45",
        &[
            "windows-x86_64-gnu.zip",
            "windows-x86_64-gnu.zip.sha256",
            "macos-aarch64.tar.gz",
            "macos-aarch64.tar.gz.sha256",
            "linux-x86_64-gnu.tar.gz",
            "linux-x86_64-gnu.tar.gz.sha256",
        ],
    );
    let direct = fixture_release(
        "1.0.110",
        &[
            "windows-x86_64-gnu.zip",
            "windows-x86_64-msvc.zip",
            "windows-aarch64-msvc.zip",
            "macos-aarch64.tar.gz",
            "macos-x86_64.tar.gz",
            "linux-x86_64-gnu.tar.gz",
            "linux-aarch64-gnu.tar.gz",
        ],
    );
    let releases = [bridge, direct];
    let (_, version) =
        select_latest_compatible_release(&releases, "stable", CommunityPlatform::WindowsX86_64Gnu)
            .unwrap();
    assert_eq!(
        version.to_string(),
        if HARNESS_EXACT_ASSET_SET {
            "1.0.45"
        } else {
            "1.0.110"
        }
    );
    if HARNESS_EXACT_ASSET_SET {
        assert!(
            select_asset_for_platform(&releases[1], "1.0.110", CommunityPlatform::WindowsX86_64Gnu)
                .is_err()
        );
    }
}
