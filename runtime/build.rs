use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    // Get compiler version info using RUSTC env var
    let rustc = env::var_os("RUSTC").expect("RUSTC env var not set");
    let output = Command::new(rustc).arg("-vV").output().expect("Failed to execute rustc");

    let version_info = String::from_utf8(output.stdout).expect("Invalid UTF-8");

    // Extract commit hash (first 9 chars)
    let commit_hash = version_info
        .lines()
        .find(|line| line.starts_with("commit-hash: "))
        .and_then(|line| line.strip_prefix("commit-hash: "))
        .map(|hash| &hash[..9])
        .expect("Failed to find rustc commit hash");

    println!("cargo:rustc-env=RUSTC_COMMIT_HASH={}", commit_hash);

    // Generate signatures module
    generate_signatures_module();
}

fn generate_signatures_module() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let signatures_path = Path::new("../signatures");

    let mut signatures_code = String::new();
    signatures_code.push_str("// Auto-generated signature mappings\n\n");
    signatures_code.push_str("pub fn get_method_signatures() -> ::std::collections::HashMap<(&'static str, &'static str), &'static str> {\n");
    signatures_code.push_str("    let mut sigs = ::std::collections::HashMap::new();\n");

    if signatures_path.exists() {
        for entry in fs::read_dir(signatures_path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("sig") {
                let struct_name = path.file_stem().unwrap().to_str().unwrap();
                let content = fs::read_to_string(&path).unwrap();

                for line in content.lines() {
                    let parts: Vec<&str> = line.splitn(3, ':').collect();
                    if parts.len() == 3 {
                        let method_name = parts[0];
                        let return_type = parts[2];
                        signatures_code.push_str(&format!(
                            "    sigs.insert((\"{}\", \"{}\"), \"{}\");\n",
                            struct_name, method_name, return_type
                        ));
                    }
                }
            }
        }
    }

    signatures_code.push_str("    sigs\n");
    signatures_code.push_str("}\n");

    let dest_path = Path::new(&out_dir).join("signatures.rs");
    fs::write(&dest_path, signatures_code).unwrap();

    // Tell cargo to rerun if signatures change
    println!("cargo:rerun-if-changed=../signatures");
}
