// Direct replacement is a transport-compatible package, not a self-extracting
// executable. These fixtures never run package code.
fn direct_protocol(version: &str) -> serde_json::Value {
    serde_json::json!({
        "schema": 1, "version": version, "platform": "x86_64-pc-windows-gnu",
        "mode": "executable-only", "manifest": INNER_MANIFEST,
        "executable": "grok-zh.exe", "installer": "Install-GrokZh.ps1",
        "direct_runtime": {"schema": 1, "version": version, "platform": "x86_64-pc-windows-msvc"}
    })
}

fn direct_write_package(
    path: &Path,
    version: &str,
    protocol: serde_json::Value,
    image: Vec<u8>,
    tamper: bool,
) {
    let mut entries = package_entries_with_manifest(&WINDOWS_REQUIRED_PACKAGE_FILES);
    bundle_set_entry(&mut entries, "grok-zh.exe", image);
    bundle_set_entry(
        &mut entries,
        BUILD_INFO,
        format!("GROK-UPDATE-PROTOCOL-BEGIN\n{protocol}\nGROK-UPDATE-PROTOCOL-END\n").into_bytes(),
    );
    rehash_entries(&mut entries);
    if tamper {
        bundle_set_entry(&mut entries, "grok-zh.exe", b"tampered".to_vec());
    }
    write_zip(
        path,
        &bundle_entries_with_root(&format!("grok-zh-{version}-windows-x86_64-gnu"), entries),
    );
}

#[test]
fn direct_runtime_passes_legacy_validation_and_two_future_msvc_updates() {
    let temp = tempfile::tempdir().unwrap();
    for version in ["1.0.110", "1.0.111", "1.0.112"] {
        let archive_path = temp.path().join(format!("{version}.zip"));
        direct_write_package(
            &archive_path,
            version,
            direct_protocol(version),
            bundle_pe_image(),
            false,
        );
        // The oldest fixed-layout updater does not parse the extension. Check
        // its complete layout and manifest contract rather than only new code.
        let root = format!("grok-zh-{version}-windows-x86_64-gnu");
        let mut archive = zip::ZipArchive::new(File::open(&archive_path).unwrap()).unwrap();
        validate_archive_layout(&mut archive, &WINDOWS_REQUIRED_PACKAGE_FILES, Some(&root))
            .unwrap();
        let hashes =
            read_inner_manifest(&mut archive, &WINDOWS_REQUIRED_PACKAGE_FILES, Some(&root))
                .unwrap();
        for (name, expected) in hashes {
            assert_eq!(
                hash_manifest_entry(&mut archive, &name, Some(&root), None).unwrap(),
                expected
            );
        }
        for platform in [
            CommunityPlatform::WindowsX86_64Gnu,
            CommunityPlatform::WindowsX86_64Msvc,
        ] {
            let mut asset = verified_windows_asset(version);
            asset.target_platform = platform;
            let destination = temp.path().join(format!("{version}-{platform:?}.exe"));
            extract_verified_executable(&asset, &archive_path, &destination).unwrap();
            assert_eq!(std::fs::read(destination).unwrap(), bundle_pe_image());
        }
    }
}

#[test]
fn direct_runtime_rejects_invalid_descriptors_images_and_hashes_without_leaving_candidate() {
    let temp = tempfile::tempdir().unwrap();
    let version = "1.0.110";
    let good = direct_protocol(version);
    let mut cases = Vec::new();
    for (field, value) in [
        ("schema", serde_json::json!(2)),
        ("version", serde_json::json!("1.0.111")),
        ("platform", serde_json::json!("aarch64-pc-windows-msvc")),
        ("unexpected", serde_json::json!(true)),
    ] {
        let mut protocol = good.clone();
        protocol["direct_runtime"][field] = value;
        cases.push((protocol, bundle_pe_image(), false));
    }
    for field in ["schema", "version", "platform"] {
        let mut protocol = good.clone();
        protocol["direct_runtime"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        cases.push((protocol, bundle_pe_image(), false));
    }
    let mut ambiguous = good.clone();
    ambiguous["embedded_runtime"] = bundle_descriptor(version, b"zip");
    cases.push((ambiguous, bundle_pe_image(), false));
    cases.push((good.clone(), b"not PE".to_vec(), false));
    cases.push((good.clone(), bundle_launcher(b"embedded ZIP"), false));
    cases.push((good.clone(), bundle_pe_image(), true));
    let mut wrong_machine = bundle_pe_image();
    wrong_machine[132..134].copy_from_slice(&0xaa64u16.to_le_bytes());
    cases.push((good, wrong_machine, false));
    for (index, (protocol, image, tamper)) in cases.into_iter().enumerate() {
        let archive = temp.path().join(format!("bad-{index}.zip"));
        direct_write_package(&archive, version, protocol, image, tamper);
        for platform in [
            CommunityPlatform::WindowsX86_64Gnu,
            CommunityPlatform::WindowsX86_64Msvc,
        ] {
            let mut asset = verified_windows_asset(version);
            asset.target_platform = platform;
            let output = temp.path().join("candidate.exe");
            assert!(
                extract_verified_executable(&asset, &archive, &output).is_err(),
                "case {index}"
            );
            assert!(!output.exists(), "candidate remained in case {index}");
        }
    }
}

#[test]
fn direct_runtime_descriptor_is_only_valid_in_gnu_transport() {
    for platform in [
        "x86_64-pc-windows-msvc",
        "aarch64-pc-windows-msvc",
        "x86_64-unknown-linux-gnu",
    ] {
        let mut protocol = direct_protocol("1.0.110");
        protocol["platform"] = serde_json::json!(platform);
        let info = format!("GROK-UPDATE-PROTOCOL-BEGIN\n{protocol}\nGROK-UPDATE-PROTOCOL-END\n");
        assert!(PackageProtocol::parse(info.as_bytes(), "1.0.110", platform).is_err());
    }
}

#[test]
fn direct_runtime_requires_descriptor_for_msvc_and_preserves_existing_target() {
    let temp = tempfile::tempdir().unwrap();
    let version = "1.0.110";
    for null in [false, true] {
        let mut protocol = direct_protocol(version);
        if null {
            protocol["direct_runtime"] = serde_json::Value::Null;
        } else {
            protocol.as_object_mut().unwrap().remove("direct_runtime");
        }
        let archive = temp.path().join("missing.zip");
        direct_write_package(&archive, version, protocol, bundle_pe_image(), false);
        let output = temp.path().join("candidate.exe");
        assert!(extract_verified_executable(&bundle_asset(version), &archive, &output).is_err());
        assert!(!output.exists());
    }
    let archive = temp.path().join("valid.zip");
    direct_write_package(
        &archive,
        version,
        direct_protocol(version),
        bundle_pe_image(),
        false,
    );
    let output = temp.path().join("existing.exe");
    std::fs::write(&output, b"retain").unwrap();
    assert!(extract_verified_executable(&bundle_asset(version), &archive, &output).is_err());
    assert_eq!(std::fs::read(output).unwrap(), b"retain");
}
