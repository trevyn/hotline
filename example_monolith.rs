// Example demonstrating monolith mode with compile-time object resolution

#[cfg(feature = "monolith")]
use runtime::TypedRuntime;
#[cfg(feature = "monolith")]
use rect::Rect;
#[cfg(feature = "monolith")]
use hotline::{TypedMessage, TypedValue, typed_send};

#[cfg(feature = "monolith")]
fn main() {
    let mut runtime = TypedRuntime::new();
    
    // With monolith feature, rect is directly available as a type
    let rect = Rect {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 50.0,
    };
    
    // Register rect - this is compile-time checked
    let handle = runtime.register_rect(rect);
    
    // Send messages - the compiler knows rect supports these methods
    let result = typed_send!(runtime, handle, get_x()).unwrap();
    println!("x: {:?}", result.get::<f64>());
    
    typed_send!(runtime, handle, move_by(5.0, 10.0)).unwrap();
    
    let result = typed_send!(runtime, handle, get_x()).unwrap();
    println!("x after move: {:?}", result.get::<f64>());
    
    // With monolith, the compiler can verify at compile time that rect
    // has these methods, even though we're still using dynamic dispatch
    // under the hood. The key is that all object types are known at
    // compile time when building with --features monolith.
}

#[cfg(not(feature = "monolith"))]
fn main() {
    println!("This example requires the monolith feature. Run with:");
    println!("cargo run --example example_monolith --features monolith");
}