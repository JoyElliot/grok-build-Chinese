fn main() {
    let python = if cfg!(windows) { "python" } else { "python3" };
    let status = std::process::Command::new(python)
        .args(["verify_sources.py", "--check"])
        .status()
        .expect("Python is required for byte-for-byte historical-source provenance validation");
    assert!(status.success(), "Historical source or generated exact-source module changed");
    println!("cargo:rerun-if-changed=historical");
    println!("cargo:rerun-if-changed=source-provenance.json");
    println!("cargo:rerun-if-changed=verify_sources.py");
    println!("cargo:rerun-if-changed=src/historical.rs");
}
