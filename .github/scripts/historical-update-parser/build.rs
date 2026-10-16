fn main() {
    let python = if cfg!(windows) { "python" } else { "python3" };
    let status = std::process::Command::new(python)
        .args(["verify_sources.py", "--check"])
        .status()
        .expect("Python is required for byte-for-byte historical-source provenance validation");
    assert!(status.success(), "Historical source or generated exact-source module changed");
    let selection_status = std::process::Command::new(python)
        .args(["selection-matrix/verify_sources.py", "--check"])
        .status()
        .expect("Python is required for exact-source selection matrix provenance");
    assert!(selection_status.success(), "Selection/current extraction source provenance changed");
    println!("cargo:rerun-if-changed=selection-matrix/sources");
    println!("cargo:rerun-if-changed=selection-matrix/src");
    println!("cargo:rerun-if-changed=selection-matrix/fixtures");
    println!("cargo:rerun-if-changed=selection-matrix/source-provenance.json");
    println!("cargo:rerun-if-changed=selection-matrix/verify_sources.py");
    println!("cargo:rerun-if-changed=historical");
    println!("cargo:rerun-if-changed=source-provenance.json");
    println!("cargo:rerun-if-changed=verify_sources.py");
    println!("cargo:rerun-if-changed=src/historical.rs");
}
