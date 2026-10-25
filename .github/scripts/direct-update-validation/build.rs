use std::path::Path;
use std::process::Command;

fn main() {
    // Keep test copies tied to this checkout; stale generated snapshots must
    // never allow tests to pass after the updater under review has changed.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for input in [
        "prepare.py",
        "historical",
        "package_compatibility.rs",
        "old_msvc_compatibility.rs",
        "../../../crates/codegen/xai-grok-update/src/community_release.rs",
        "../../../crates/codegen/xai-grok-update/src/community_release",
        "../../../crates/codegen/xai-grok-update/src/community_update_cancel.rs",
    ] {
        println!("cargo:rerun-if-changed={}", root.join(input).display());
    }
    let status = Command::new("python")
        .arg(root.join("prepare.py"))
        .status()
        .expect("Python is required for exact-source provenance verification");
    assert!(
        status.success(),
        "exact-source preparation or integrity check failed"
    );
}
