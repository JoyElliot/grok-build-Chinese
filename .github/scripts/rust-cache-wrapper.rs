//! Cache workspace libraries only; keep Cargo's compiler arguments unchanged.
use std::env;
use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

fn library(args: &[OsString]) -> bool {
    if args.iter().any(|arg| arg == "--test") {
        return false;
    }
    let mut found = false;
    for (index, arg) in args.iter().enumerate() {
        let value = arg.to_str().unwrap_or("");
        let kind = if value == "--crate-type" {
            args.get(index + 1).and_then(|arg| arg.to_str())
        } else {
            value.strip_prefix("--crate-type=")
        };
        if let Some(kind) = kind {
            found = true;
            if !kind.split(',').all(|kind| matches!(kind, "lib" | "rlib")) {
                return false;
            }
        } else if value == "--crate-type" {
            return false;
        }
    }
    found
}

fn inside_workspace() -> bool {
    let workspace =
        env::var_os("GITHUB_WORKSPACE").and_then(|path| Path::new(&path).canonicalize().ok());
    let manifest =
        env::var_os("CARGO_MANIFEST_DIR").and_then(|path| Path::new(&path).canonicalize().ok());
    match (workspace, manifest) {
        (Some(workspace), Some(manifest)) => manifest.starts_with(workspace),
        _ => false,
    }
}

fn main() {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    let Some(compiler) = args.first() else {
        eprintln!("missing rustc executable");
        std::process::exit(2);
    };
    let cache =
        env::var_os("GROK_SCCACHE_EXE").filter(|_| inside_workspace() && library(&args[1..]));
    let result = match cache {
        Some(cache) => Command::new(cache).args(&args).status().or_else(|error| {
            // Failure to launch the optional cache executable is not a compiler
            // failure. A started compiler's nonzero exit is never retried/hidden.
            eprintln!("compiler cache unavailable ({error}); invoking rustc directly");
            Command::new(compiler).args(&args[1..]).status()
        }),
        None => Command::new(compiler).args(&args[1..]).status(),
    };
    match result {
        // Preserve Windows NTSTATUS values too: narrowing/clamping a negative
        // i32 status could otherwise turn a compiler crash into exit 0.
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(error) => {
            eprintln!("cannot invoke rustc: {error}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::library;
    use std::ffi::OsString;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn only_plain_libraries_are_cached() {
        assert!(library(&args(&["--crate-type", "lib"])));
        assert!(library(&args(&["--crate-type=rlib"])));
        assert!(!library(&args(&["--crate-type=proc-macro"])));
        assert!(!library(&args(&["--crate-type=cdylib"])));
        assert!(!library(&args(&["--crate-type=lib,bin"])));
        assert!(!library(&args(&[
            "--crate-type=lib",
            "--crate-type=proc-macro"
        ])));
        assert!(!library(&args(&[
            "--crate-type",
            "lib",
            "--crate-type",
            "bin"
        ])));
        assert!(!library(&args(&["--crate-type=lib", "--crate-type"])));
        assert!(!library(&args(&["--crate-type=lib", "--test"])));
        assert!(!library(&args(&["--crate-type"])));
        assert!(!library(&args(&["-vV"])));
    }
}
