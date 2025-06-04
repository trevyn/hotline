// Minimal test to profile library loading times
use runtime::DirectRuntime;
use std::path::Path;
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let use_dlopen = std::env::args().nth(1).as_deref() == Some("--dlopen");
    
    if use_dlopen {
        println!("Testing library load times with dlopen...\n");
    } else {
        println!("Testing library load times with CUSTOM LOADER...\n");
    }
    
    let mut runtime = if use_dlopen {
        DirectRuntime::new()
    } else {
        DirectRuntime::new_with_custom_loader()
    };
    let objects_dir = Path::new("objects");
    
    let total_start = Instant::now();
    
    // Discover libraries
    let libs: Vec<(String, String)> = std::fs::read_dir(objects_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let lib_name = path.file_name()?.to_str()?.to_string();
            
            #[cfg(target_os = "macos")]
            let lib_path = format!("target/release/lib{}.dylib", lib_name);
            #[cfg(target_os = "linux")]
            let lib_path = format!("target/release/lib{}.so", lib_name);
            #[cfg(target_os = "windows")]
            let lib_path = format!("target/release/{}.dll", lib_name);
            
            if Path::new(&lib_path).exists() {
                Some((lib_name, lib_path))
            } else {
                None
            }
        })
        .collect();
    
    // Load each library
    for (lib_name, lib_path) in &libs {
        let start = Instant::now();
        
        if let Err(e) = runtime.hot_reload(lib_path, lib_name) {
            eprintln!("Failed to load {}: {}", lib_name, e);
        } else {
            let elapsed = start.elapsed();
            println!("Loaded {}: {:.1}ms", lib_name, elapsed.as_secs_f64() * 1000.0);
        }
    }
    
    let total = total_start.elapsed();
    println!("\nTotal: {:.1}ms for {} libraries", total.as_secs_f64() * 1000.0, libs.len());
    println!("Average: {:.1}ms per library", total.as_secs_f64() * 1000.0 / libs.len() as f64);
    
    Ok(())
}