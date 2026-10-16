fn main() {
    let python = if cfg!(windows) { "python" } else { "python3" };
    let status = std::process::Command::new(python)
        .args(["verify_sources.py", "--check"])
        .status()
        .expect("Python is required for exact-source provenance validation");
    assert!(status.success(), "Pinned sources or generated selection modules changed");
    for path in ["sources", "source-provenance.json", "verify_sources.py", "fixtures", "src"] {
        println!("cargo:rerun-if-changed={path}");
    }
}
