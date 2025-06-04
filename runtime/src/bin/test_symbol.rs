// Test calling a symbol from a custom loaded library
use runtime::DirectRuntime;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let use_dlopen = std::env::args().nth(1).as_deref() == Some("--dlopen");
    
    println!("Testing symbol calls with {} loader...\n", 
        if use_dlopen { "dlopen" } else { "CUSTOM" });
    
    let mut runtime = if use_dlopen {
        DirectRuntime::new()
    } else {
        DirectRuntime::new_with_custom_loader()
    };
    
    // Load Rect library
    let lib_path = "target/release/libRect.dylib";
    let lib_name = "Rect";
    
    println!("Loading {}...", lib_path);
    runtime.hot_reload(lib_path, lib_name)?;
    
    // Try to create a Rect object
    println!("\nTrying to create Rect object...");
    match runtime.create_from_lib("libRect", "Rect") {
        Ok(handle) => {
            println!("Successfully created Rect object!");
            
            // Try to call a method
            println!("\nTrying to call bounds() method...");
            match runtime.call_method(
                &handle,
                "Rect",
                "libRect",
                "bounds",
                vec![]
            ) {
                Ok(result) => {
                    println!("Successfully called bounds(): {:?}", result);
                },
                Err(e) => {
                    println!("Failed to call bounds(): {}", e);
                }
            }
        },
        Err(e) => {
            println!("Failed to create Rect: {}", e);
        }
    }
    
    Ok(())
}