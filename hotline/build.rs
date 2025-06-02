use std::env;
use std::process::Command;

fn main() {
    // Get compiler version info using RUSTC env var
    let rustc = env::var_os("RUSTC").expect("RUSTC env var not set");
    let output = Command::new(rustc)
        .arg("-vV")
        .output()
        .expect("Failed to execute rustc");
    
    let version_info = String::from_utf8(output.stdout).expect("Invalid UTF-8");
    
    // Extract commit hash (first 9 chars)
    let commit_hash = version_info
        .lines()
        .find(|line| line.starts_with("commit-hash: "))
        .and_then(|line| line.strip_prefix("commit-hash: "))
        .map(|hash| &hash[..9])
        .expect("Failed to find rustc commit hash");
    
    println!("cargo:rustc-env=RUSTC_COMMIT_HASH={}", commit_hash);
}