fn main() {
    let python = if cfg!(windows) { "python" } else { "python3" };
    assert!(std::process::Command::new(python).arg("prepare.py").status().expect("Python required only when building the harness").success());
    println!("cargo:rerun-if-changed=prepare.py");
    println!("cargo:rerun-if-changed=historical");
    println!("cargo:rerun-if-changed=historical-pins.json");
    println!("cargo:rerun-if-changed=regressions");
    println!("cargo:rerun-if-changed=../../../crates/codegen/xai-grok-update/src");
    println!("cargo:rerun-if-changed=../../../crates/codegen/xai-grok-product/src/lib.rs");
    println!("cargo:rerun-if-changed=../../../crates/codegen/xai-tty-utils/src/lib.rs");
}
