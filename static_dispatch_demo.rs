// Demo showing full static dispatch with zero virtual calls in monolith mode

#[cfg(feature = "monolith")]
use hotline::{TypedMessage, TypedValue, static_call, rect_call};
#[cfg(feature = "monolith")]
use runtime::{StaticRuntime, AllObjects};

#[cfg(feature = "monolith")]
fn main() {
    println!("=== Static Dispatch Demo (Zero Virtual Calls) ===\n");
    
    let mut runtime = StaticRuntime::new();
    
    // Create rect - directly as enum variant
    let rect = hotline::monolith::rect::Rect {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 50.0,
    };
    
    // Register - no boxing, stored directly in enum
    let handle = runtime.register(AllObjects::Rect(rect));
    
    // Method 1: Using the typed_send macro (still goes through message dispatch)
    println!("Method 1: typed_send (message dispatch)");
    let msg = TypedMessage {
        selector: "get_x".to_string(),
        args: vec![],
    };
    let result = runtime.send(handle, msg).unwrap();
    println!("x = {:?}", result.get::<f64>());
    
    // Method 2: Using static_call macro (compiles to direct method call)
    println!("\nMethod 2: static_call (zero-cost dispatch)");
    let result = static_call!(runtime, handle, get_y()).unwrap();
    println!("y = {:?}", result.get::<f64>());
    
    // Method 3: Using rect_call when you know it's a rect (most direct)
    println!("\nMethod 3: rect_call (type-specific, most direct)");
    let result = rect_call!(runtime, handle, get_width()).unwrap();
    println!("width = {:?}", result.get::<f64>());
    
    // Method 4: Direct access to the rect (no TypedValue wrapper)
    println!("\nMethod 4: Direct access (no wrapper overhead)");
    if let Some(rect) = runtime.get_rect(handle) {
        println!("Direct access: x={}, y={}, w={}, h={}", 
                 rect.x, rect.y, rect.width, rect.height);
    }
    
    // Mutate using static dispatch
    static_call!(runtime, handle, move_by(15.0, 25.0)).unwrap();
    
    // Verify mutation
    let x = rect_call!(runtime, handle, get_x()).unwrap();
    let y = rect_call!(runtime, handle, get_y()).unwrap();
    println!("\nAfter move_by(15, 25): x={:?}, y={:?}", 
             x.get::<f64>(), y.get::<f64>());
    
    println!("\n=== Key Benefits ===");
    println!("1. No Box<dyn TypedObject> allocations");
    println!("2. No vtable lookups - all calls can be inlined");
    println!("3. Enum variants stored inline in Vec");
    println!("4. Compiler knows all possible types at compile time");
    println!("5. Direct field access possible without Any downcasting");
}

#[cfg(not(feature = "monolith"))]
fn main() {
    println!("This demo requires the monolith feature. Run with:");
    println!("cargo run --bin static_dispatch_demo --features monolith");
}