use std::env;
use std::process::Command;
use std::str;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let version = match rustc_version() {
        Some(version) => version,
        None => return,
    };

    if version.minor >= 80 {
        println!("cargo:rustc-check-cfg=cfg(no_literal_fromstr)");
        println!("cargo:rustc-check-cfg=cfg(feature, values(\"protocol_feature_paste\"))");
    }

    if version.minor < 54 {
        // https://github.com/rust-lang/rust/pull/84717
        println!("cargo:rustc-cfg=no_literal_fromstr");
    }
    
    // Extract rustc commit hash
    if let Some(commit_hash) = rustc_commit_hash() {
        println!("cargo:rustc-env=RUSTC_COMMIT_HASH={}", commit_hash);
    }
}

struct RustcVersion {
    minor: u32,
}

fn rustc_version() -> Option<RustcVersion> {
    let rustc = env::var_os("RUSTC")?;
    let output = Command::new(rustc).arg("--version").output().ok()?;
    let version = str::from_utf8(&output.stdout).ok()?;
    let mut pieces = version.split('.');
    if pieces.next() != Some("rustc 1") {
        return None;
    }
    let minor = pieces.next()?.parse().ok()?;
    Some(RustcVersion { minor })
}

fn rustc_commit_hash() -> Option<String> {
    let rustc = env::var_os("RUSTC")?;
    let output = Command::new(rustc).arg("-vV").output().ok()?;
    let version_info = str::from_utf8(&output.stdout).ok()?;
    
    // Extract commit hash (first 9 chars)
    version_info
        .lines()
        .find(|line| line.starts_with("commit-hash: "))
        .and_then(|line| line.strip_prefix("commit-hash: "))
        .map(|hash| hash.chars().take(9).collect())
}
