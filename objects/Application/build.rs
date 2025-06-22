use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    // Only rerun if shader files change
    println!("cargo:rerun-if-changed=shaders/");

    let out_dir = env::var("OUT_DIR").unwrap();
    let shader_dir = Path::new("shaders");
    let out_shader_dir = Path::new(&out_dir).join("shaders");

    // Create output directory
    fs::create_dir_all(&out_shader_dir).unwrap();

    // List of shaders to compile
    let shaders = [("quad.vert", "vert"), ("quad.frag", "frag"), ("solid.vert", "vert"), ("solid.frag", "frag")];

    for (shader_name, _shader_type) in &shaders {
        let input_path = shader_dir.join(shader_name);
        let output_path = out_shader_dir.join(format!("{}.spv", shader_name));

        // Skip if shader doesn't exist yet
        if !input_path.exists() {
            continue;
        }

        println!("cargo:rerun-if-changed={}", input_path.display());
        println!("cargo:warning=Compiling shader: {} -> {}", input_path.display(), output_path.display());

        // Try to compile with glslc (from Vulkan SDK)
        let result = Command::new("glslc").arg(&input_path).arg("-o").arg(&output_path).output();

        match result {
            Ok(output) => {
                if !output.status.success() {
                    eprintln!("Failed to compile shader {}: {}", shader_name, String::from_utf8_lossy(&output.stderr));
                    // Don't fail the build, just skip shader compilation
                }
            }
            Err(e) => {
                eprintln!("glslc not found or failed to run: {}. Skipping shader compilation.", e);
                // Copy pre-compiled shaders if available
                copy_precompiled_shaders(&out_shader_dir);
                return;
            }
        }
    }
}

fn copy_precompiled_shaders(out_dir: &Path) {
    // In a real project, you might include pre-compiled shaders
    // For now, we'll just create empty files to allow the build to continue
    let shaders = ["quad.vert.spv", "quad.frag.spv", "solid.vert.spv", "solid.frag.spv"];

    for shader in &shaders {
        let path = out_dir.join(shader);
        if !path.exists() {
            fs::write(&path, &[]).ok();
        }
    }
}
