// Included in the release test module so these fixtures exercise exactly the
// same archive validator and historical release-selection helpers.

fn bundle_zip_bytes(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in entries {
        writer.start_file(name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn bundle_native_entries(version: &str, target: &str) -> Vec<(String, Vec<u8>)> {
    let mut entries = package_entries_with_manifest(&WINDOWS_REQUIRED_PACKAGE_FILES);
    bundle_set_entry(
        &mut entries,
        BUILD_INFO,
        protocol_info(version, target, "grok-zh.exe", "Install-GrokZh.ps1"),
    );
    bundle_set_entry(&mut entries, "grok-zh.exe", bundle_pe_image());
    rehash_entries(&mut entries);
    entries
}

fn bundle_set_entry(entries: &mut [(String, Vec<u8>)], name: &str, bytes: Vec<u8>) {
    entries
        .iter_mut()
        .find(|(path, _)| path == name)
        .unwrap()
        .1 = bytes;
}

fn bundle_entries_with_root(root: &str, entries: Vec<(String, Vec<u8>)>) -> Vec<(String, Vec<u8>)> {
    entries
        .into_iter()
        .map(|(name, bytes)| (format!("{root}/{name}"), bytes))
        .collect()
}

fn bundle_native_zip(version: &str, entries: Vec<(String, Vec<u8>)>) -> Vec<u8> {
    let root = format!("grok-zh-{version}-windows-x86_64-msvc");
    bundle_zip_bytes(&bundle_entries_with_root(&root, entries))
}

fn bundle_pe_image() -> Vec<u8> {
    let mut bytes = vec![0u8; 512];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[60..64].copy_from_slice(&128u32.to_le_bytes());
    bytes[128..134].copy_from_slice(b"PE\0\0\x64\x86");
    bytes[134..136].copy_from_slice(&1u16.to_le_bytes());
    bytes[148..150].copy_from_slice(&240u16.to_le_bytes());
    bytes[152..154].copy_from_slice(&0x20bu16.to_le_bytes());
    bytes
}

fn bundle_append(mut prefix: Vec<u8>, inner: &[u8]) -> Vec<u8> {
    let offset = prefix.len() as u64;
    prefix.extend_from_slice(inner);
    prefix.extend_from_slice(b"GROKMSVCBUNDLE01");
    prefix.extend_from_slice(&offset.to_le_bytes());
    prefix.extend_from_slice(&(inner.len() as u64).to_le_bytes());
    prefix.extend_from_slice(&Sha256::digest(inner));
    prefix
}

fn bundle_launcher(inner: &[u8]) -> Vec<u8> {
    bundle_append(bundle_pe_image(), inner)
}

fn bundle_descriptor(version: &str, inner: &[u8]) -> serde_json::Value {
    serde_json::json!({
        "schema": 1,
        "format": "grok-msvc-bundle-v1",
        "version": version,
        "platform": "x86_64-pc-windows-msvc",
        "archive_size": inner.len(),
        "archive_sha256": sha256_hex(inner)
    })
}

fn bundle_write_outer(
    path: &Path,
    version: &str,
    launcher: Vec<u8>,
    descriptor: Option<serde_json::Value>,
) {
    let mut entries = package_entries_with_manifest(&WINDOWS_REQUIRED_PACKAGE_FILES);
    bundle_set_entry(&mut entries, "grok-zh.exe", launcher);
    let mut protocol = serde_json::json!({
        "schema": 1, "version": version, "platform": "x86_64-pc-windows-gnu",
        "mode": "executable-only", "manifest": INNER_MANIFEST,
        "executable": "grok-zh.exe", "installer": "Install-GrokZh.ps1"
    });
    if let Some(descriptor) = descriptor {
        protocol["embedded_runtime"] = descriptor;
    }
    bundle_set_entry(
        &mut entries,
        BUILD_INFO,
        format!("GROK-UPDATE-PROTOCOL-BEGIN\n{protocol}\nGROK-UPDATE-PROTOCOL-END\n").into_bytes(),
    );
    rehash_entries(&mut entries);
    let root = format!("grok-zh-{version}-windows-x86_64-gnu");
    write_zip(path, &bundle_entries_with_root(&root, entries));
}

fn bundle_asset(version: &str) -> VerifiedAsset {
    let mut asset = verified_windows_asset(version);
    asset.target_platform = CommunityPlatform::WindowsX86_64Msvc;
    asset
}

#[test]
fn embedded_wrapper_installs_only_native_executable_across_two_releases() {
    let temp = tempfile::tempdir().unwrap();
    for version in ["1.0.99", "1.0.100"] {
        let inner = bundle_native_zip(
            version,
            bundle_native_entries(version, "x86_64-pc-windows-msvc"),
        );
        let launcher = bundle_launcher(&inner);
        let path = temp.path().join(format!("{version}.zip"));
        bundle_write_outer(
            &path,
            version,
            launcher.clone(),
            Some(bundle_descriptor(version, &inner)),
        );
        let native = temp.path().join(format!("{version}.native.exe"));
        extract_verified_executable(&bundle_asset(version), &path, &native).unwrap();
        assert_eq!(std::fs::read(&native).unwrap(), bundle_pe_image());
        assert!(!native.with_extension("embedded-launcher.tmp").exists());
        // The original GNU updater still extracts exactly the launcher that
        // will perform the first migration. No ABI is inferred from filenames.
        let legacy = temp.path().join(format!("{version}.gnu.exe"));
        extract_verified_executable(&verified_windows_asset(version), &path, &legacy).unwrap();
        assert_eq!(std::fs::read(legacy).unwrap(), launcher);
    }
}

#[test]
fn unrecognized_gnu_packages_are_never_installed_over_msvc() {
    let version = "1.0.99";
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("historical-gnu.zip");
    let candidate = temp.path().join("candidate.exe");
    bundle_write_outer(&path, version, b"historical GNU executable".to_vec(), None);
    let error =
        extract_verified_executable(&bundle_asset(version), &path, &candidate).unwrap_err();
    assert!(error.to_string().contains("full installer"));
    assert!(!candidate.exists());
    assert!(!candidate.with_extension("embedded-launcher.tmp").exists());
}

#[test]
fn historical_gnu_without_protocol_cannot_replace_msvc() {
    let version = "1.0.99";
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("legacy-without-protocol.zip");
    write_zip(
        &path,
        &nested_package_entries(version, &WINDOWS_REQUIRED_PACKAGE_FILES),
    );
    let candidate = temp.path().join("candidate.exe");
    let error =
        extract_verified_executable(&bundle_asset(version), &path, &candidate).unwrap_err();
    assert!(error.to_string().contains("full GNU installer"));
    assert!(!candidate.exists());
    assert!(!candidate.with_extension("embedded-launcher.tmp").exists());
}

#[test]
fn embedded_outer_manifest_tamper_is_rejected_before_native_activation() {
    let version = "1.0.99";
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("outer-tamper.zip");
    let inner = bundle_native_zip(
        version,
        bundle_native_entries(version, "x86_64-pc-windows-msvc"),
    );
    bundle_write_outer(
        &path,
        version,
        bundle_launcher(&inner),
        Some(bundle_descriptor(version, &inner)),
    );
    let mut archive = zip::ZipArchive::new(File::open(&path).unwrap()).unwrap();
    let mut entries = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let name = entry.name().to_string();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if name.ends_with("/rg.exe") {
            bytes.push(0);
        }
        entries.push((name, bytes));
    }
    drop(archive);
    // The outer API digest is assumed verified by the download layer. A valid
    // API hash never substitutes for its complete declared file manifest.
    write_zip(&path, &entries);
    let candidate = temp.path().join("candidate.exe");
    assert!(extract_verified_executable(&bundle_asset(version), &path, &candidate).is_err());
    assert!(!candidate.exists());
    assert!(!candidate.with_extension("embedded-launcher.tmp").exists());
}

#[test]
fn embedded_bundle_rejects_invalid_descriptors_footer_ranges_and_digests() {
    let version = "1.0.99";
    let inner = bundle_native_zip(
        version,
        bundle_native_entries(version, "x86_64-pc-windows-msvc"),
    );
    let valid_launcher = bundle_launcher(&inner);
    let valid_descriptor = bundle_descriptor(version, &inner);
    let temp = tempfile::tempdir().unwrap();
    for case in [
        "format",
        "schema",
        "version",
        "platform",
        "size",
        "limit",
        "digest",
        "magic",
        "offset",
        "overflow",
        "trailing",
        "payload",
        "pe",
        "zip-prefix",
        "pe32",
        "machine",
        "pe-boundary",
        "short-prefix",
        "optional-size-zero",
        "optional-overlap",
        "optional-too-short",
        "sections-zero",
        "sections-overlap",
    ] {
        let mut launcher = valid_launcher.clone();
        let mut descriptor = valid_descriptor.clone();
        let footer = launcher.len() - 64;
        match case {
            "format" => descriptor["format"] = "unknown-bundle".into(),
            "schema" => descriptor["schema"] = 2.into(),
            "version" => descriptor["version"] = "1.0.98".into(),
            "platform" => descriptor["platform"] = "x86_64-pc-windows-gnu".into(),
            "size" => descriptor["archive_size"] = (inner.len() as u64 + 1).into(),
            "limit" => descriptor["archive_size"] = (MAX_ASSET_BYTES + 1).into(),
            "digest" => descriptor["archive_sha256"] = "00".repeat(32).into(),
            "magic" => launcher[footer] ^= 1,
            "offset" => {
                launcher[footer + 16..footer + 24].copy_from_slice(&255u64.to_le_bytes());
            }
            "overflow" => {
                launcher[footer + 16..footer + 24].copy_from_slice(&u64::MAX.to_le_bytes());
            }
            "trailing" => launcher.push(0),
            "payload" => launcher[516] ^= 1,
            "pe" => launcher[0] = b'X',
            "pe32" => launcher[152..154].copy_from_slice(&0x10bu16.to_le_bytes()),
            "machine" => launcher[132..134].copy_from_slice(&0x14cu16.to_le_bytes()),
            "pe-boundary" => launcher[60..64].copy_from_slice(&496u32.to_le_bytes()),
            "optional-size-zero" => launcher[148..150].copy_from_slice(&0u16.to_le_bytes()),
            "optional-overlap" => launcher[148..150].copy_from_slice(&480u16.to_le_bytes()),
            "optional-too-short" => launcher[148..150].copy_from_slice(&96u16.to_le_bytes()),
            "sections-zero" => launcher[134..136].copy_from_slice(&0u16.to_le_bytes()),
            "sections-overlap" => launcher[134..136].copy_from_slice(&10u16.to_le_bytes()),
            "short-prefix" => {
                launcher = bundle_append(bundle_pe_image()[..255].to_vec(), &inner);
            }
            "zip-prefix" => {
                let mut bad = inner.clone();
                bad[0] = b'X';
                launcher = bundle_launcher(&bad);
                descriptor = bundle_descriptor(version, &bad);
            }
            _ => unreachable!(),
        }
        let path = temp.path().join(format!("{case}.zip"));
        bundle_write_outer(&path, version, launcher, Some(descriptor));
        let candidate = temp.path().join(format!("{case}.exe"));
        assert!(
            extract_verified_executable(&bundle_asset(version), &path, &candidate).is_err(),
            "accepted {case}"
        );
        assert!(!candidate.exists(), "left a candidate after {case}");
        assert!(!candidate.with_extension("embedded-launcher.tmp").exists());
    }
}

#[test]
fn authenticated_inner_zip_still_requires_matching_native_protocol_and_safe_layout() {
    let version = "1.0.99";
    let temp = tempfile::tempdir().unwrap();
    for case in [
        "abi",
        "version",
        "missing-protocol",
        "tamper",
        "traversal",
        "bomb",
        "native-not-pe",
        "native-machine",
        "native-pe32",
        "native-sections",
    ] {
        let mut entries = bundle_native_entries(version, "x86_64-pc-windows-msvc");
        match case {
            "abi" | "version" | "missing-protocol" => {
                let info = match case {
                    "abi" => protocol_info(
                        version,
                        "x86_64-pc-windows-gnu",
                        "grok-zh.exe",
                        "Install-GrokZh.ps1",
                    ),
                    "version" => protocol_info(
                        "1.0.98",
                        "x86_64-pc-windows-msvc",
                        "grok-zh.exe",
                        "Install-GrokZh.ps1",
                    ),
                    _ => b"legacy build metadata".to_vec(),
                };
                bundle_set_entry(&mut entries, BUILD_INFO, info);
                rehash_entries(&mut entries);
            }
            "native-not-pe" | "native-machine" | "native-pe32" | "native-sections" => {
                let mut executable = bundle_pe_image();
                match case {
                    "native-not-pe" => executable[0] = b'X',
                    "native-machine" => {
                        executable[132..134].copy_from_slice(&0xaa64u16.to_le_bytes());
                    }
                    "native-pe32" => {
                        executable[152..154].copy_from_slice(&0x10bu16.to_le_bytes());
                    }
                    _ => executable[134..136].copy_from_slice(&10u16.to_le_bytes()),
                }
                bundle_set_entry(&mut entries, "grok-zh.exe", executable);
                rehash_entries(&mut entries);
            }
            // Deliberately keep the original manifest, while correctly hashing
            // the complete malicious ZIP in both outer integrity layers.
            "tamper" => {
                bundle_set_entry(&mut entries, "grok-zh.exe", b"tampered executable".to_vec());
            }
            "traversal" => entries.push(("../escape.exe".into(), b"escape".to_vec())),
            "bomb" => {}
            _ => unreachable!(),
        }
        let mut inner = bundle_native_zip(version, entries);
        if case == "bomb" {
            let central = inner
                .windows(4)
                .position(|bytes| bytes == b"PK\x01\x02")
                .unwrap();
            inner[central + 24..central + 28]
                .copy_from_slice(&((MAX_ENTRY_BYTES + 1) as u32).to_le_bytes());
        }
        let path = temp.path().join(format!("{case}.zip"));
        bundle_write_outer(
            &path,
            version,
            bundle_launcher(&inner),
            Some(bundle_descriptor(version, &inner)),
        );
        let candidate = temp.path().join(format!("{case}.exe"));
        assert!(
            extract_verified_executable(&bundle_asset(version), &path, &candidate).is_err(),
            "accepted inner {case}"
        );
        assert!(!candidate.exists());
        assert!(!candidate.with_extension("embedded-launcher.tmp").exists());
        assert!(!temp.path().join("escape.exe").exists());
    }
}
