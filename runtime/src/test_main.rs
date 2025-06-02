use runtime::{DirectRuntime};
use hotline::HotlineObject;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting test...");
    
    // Step 1: Create runtime
    let mut runtime = DirectRuntime::new();
    println!("Created runtime");
    
    // Step 2: Clone library registry 
    let registry = runtime.library_registry().clone();
    println!("Cloned registry");
    
    // Step 3: Set it globally
    hotline::set_library_registry(registry);
    println!("Set registry globally");
    
    // Step 4: Build WindowManager
    println!("Building WindowManager...");
    std::process::Command::new("cargo")
        .args(&["build", "--release", "-p", "WindowManager"])
        .status()?;
    println!("Build complete");
    
    // Step 5: Load library
    #[cfg(target_os = "macos")]
    let wm_path = "target/release/libWindowManager.dylib";
    
    println!("Loading library from: {}", wm_path);
    runtime.hot_reload(wm_path)?;
    println!("Library loaded");
    
    // Step 6: Create instance
    println!("Creating WindowManager instance...");
    let window_manager = runtime.create_from_lib("libWindowManager", "WindowManager")?;
    println!("Instance created");
    
    // Step 7: Test lock
    println!("Testing lock...");
    if let Ok(guard) = window_manager.lock() {
        let obj = &**guard;
        println!("Got object, type: {}", obj.type_name());
    }
    println!("Lock test complete");
    
    println!("All tests passed!");
    Ok(())
}