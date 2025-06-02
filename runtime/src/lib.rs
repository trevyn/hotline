use hotline::ObjectHandle;
use libloading::{Library, Symbol};
use std::any::Any;
use std::collections::HashMap;

pub const RUSTC_COMMIT: &str = env!("RUSTC_COMMIT_HASH");

pub mod shim_gen;

pub struct DirectRuntime {
    objects: HashMap<ObjectHandle, Box<dyn Any>>,
    next_handle: u64,
    loaded_libs: HashMap<String, Library>,
}

impl DirectRuntime {
    pub fn new() -> Self {
        Self { objects: HashMap::new(), next_handle: 1, loaded_libs: HashMap::new() }
    }

    fn type_name_for_symbol<T: 'static>() -> &'static str {
        // Always use fully qualified type names for unambiguous type safety
        std::any::type_name::<T>()
    }

    pub fn register(&mut self, obj: Box<dyn Any>) -> ObjectHandle {
        let handle = ObjectHandle(self.next_handle);
        self.next_handle += 1;
        self.objects.insert(handle, obj);
        handle
    }

    pub fn get_object(&self, handle: ObjectHandle) -> Option<&dyn Any> {
        self.objects.get(&handle).map(|b| &**b)
    }

    pub fn get_object_mut(&mut self, handle: ObjectHandle) -> Option<&mut dyn Any> {
        self.objects.get_mut(&handle).map(|b| &mut **b)
    }

    pub fn hot_reload(&mut self, lib_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let lib = unsafe { Library::new(lib_path)? };
        let lib_name = std::path::Path::new(lib_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("invalid lib path")?;

        // store the library
        self.loaded_libs.insert(lib_name.to_string(), lib);
        Ok(())
    }

    // Create object from loaded library
    pub fn create_from_lib(
        &mut self,
        lib_name: &str,
        type_name: &str,
    ) -> Result<ObjectHandle, Box<dyn std::error::Error>> {
        let lib = self.loaded_libs.get(lib_name).ok_or("library not loaded")?;

        // Call constructor with signature-encoded name
        let constructor_symbol = format!("{}__new____to__Box_lt_dyn_Any_gt__{}", type_name, RUSTC_COMMIT);
        type ConstructorFn = unsafe extern "Rust" fn() -> Box<dyn Any>;
        let constructor: Symbol<ConstructorFn> = unsafe { lib.get(constructor_symbol.as_bytes())? };

        let obj = unsafe { constructor() };
        let handle = self.register(obj);
        println!("Created object with handle: {:?}", handle);
        Ok(handle)
    }

    // Direct method calls
    pub fn call_getter<T>(
        &self,
        handle: ObjectHandle,
        type_name: &str,
        lib_name: &str,
        method: &str,
    ) -> Result<T, Box<dyn std::error::Error>>
    where
        T: Clone + 'static,
    {
        let obj = self.get_object(handle).ok_or("object not found")?;

        let lib = self.loaded_libs.get(lib_name).ok_or("library not loaded")?;

        // TODO: Ideally we'd compute this once and cache
        let type_str = Self::type_name_for_symbol::<T>();
        let symbol_name = format!("{}__get_{}____obj_ref_dyn_Any__to__{}__{}", 
            type_name, method, type_str, RUSTC_COMMIT);
        
        println!("Looking for getter symbol: {}", symbol_name);
        type GetterFn<T> = unsafe extern "Rust" fn(&dyn Any) -> T;
        let getter_fn: Symbol<GetterFn<T>> = unsafe { lib.get(symbol_name.as_bytes())? };
        
        Ok(unsafe { getter_fn(obj) })
    }

    pub fn call_setter<T>(
        &mut self,
        handle: ObjectHandle,
        type_name: &str,
        lib_name: &str,
        method: &str,
        value: T,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        T: 'static + std::fmt::Debug,
    {
        // Get symbol first to avoid borrow issues
        // Extract field name from setter method (set_x -> x)
        let field_name = method.strip_prefix("set_").unwrap_or(method);
        let value_type = Self::type_name_for_symbol::<T>();
        let symbol_name = format!(
            "{}__set_{}____obj_mut_dyn_Any__{}_{}__to__unit__{}", 
            type_name, field_name, field_name, value_type, RUSTC_COMMIT
        );
        println!("Looking for setter symbol: {} in library: {}", symbol_name, lib_name);
        type SetterFn<T> = unsafe extern "Rust" fn(&mut dyn Any, T);

        let setter_fn = {
            let lib = self.loaded_libs.get(lib_name).ok_or("library not loaded")?;
            match unsafe { lib.get::<SetterFn<T>>(symbol_name.as_bytes()) } {
                Ok(setter) => *setter,
                Err(e) => {
                    eprintln!("Failed to find symbol {}: {:?}", symbol_name, e);
                    return Err(format!("Symbol not found: {}", symbol_name).into());
                }
            }
        };

        let obj = self.get_object_mut(handle).ok_or("object not found")?;

        println!("Calling setter {}::{} with value {:?}", type_name, method, value);
        unsafe { setter_fn(obj, value) };
        Ok(())
    }

    pub fn call_method(
        &mut self,
        handle: ObjectHandle,
        type_name: &str,
        lib_name: &str,
        method: &str,
        args: Vec<Box<dyn Any>>,
    ) -> Result<Box<dyn Any>, Box<dyn std::error::Error>> {
        // Handle WindowManager methods
        if type_name == "WindowManager" {
            match method {
                "get_rects_count" if args.is_empty() => {
                    let symbol_name = format!("WindowManager__get_rects_count______obj_mut_dyn_Any____to__i64__{}", RUSTC_COMMIT);
                    type GetCountFn = unsafe extern "Rust" fn(&mut dyn Any) -> i64;
                    let getter_fn = {
                        let lib = self.loaded_libs.get(lib_name).ok_or("library not loaded")?;
                        let getter: Symbol<GetCountFn> = unsafe { lib.get(symbol_name.as_bytes())? };
                        *getter
                    };
                    let obj = self.get_object_mut(handle).ok_or("object not found")?;
                    let result = unsafe { getter_fn(obj) };
                    return Ok(Box::new(result));
                }
                "get_rect_at" if args.len() == 1 => {
                    let index = *args[0].downcast_ref::<i64>().ok_or("arg 0 not i64")?;
                    let symbol_name = format!("WindowManager__get_rect_at______obj_mut_dyn_Any____index__i64____to__i64__{}", RUSTC_COMMIT);
                    type GetAtFn = unsafe extern "Rust" fn(&mut dyn Any, i64) -> i64;
                    let getter_fn = {
                        let lib = self.loaded_libs.get(lib_name).ok_or("library not loaded")?;
                        let getter: Symbol<GetAtFn> = unsafe { lib.get(symbol_name.as_bytes())? };
                        *getter
                    };
                    let obj = self.get_object_mut(handle).ok_or("object not found")?;
                    let result = unsafe { getter_fn(obj, index) };
                    return Ok(Box::new(result));
                }
                "is_dragging" if args.is_empty() => {
                    let symbol_name = format!("WindowManager__is_dragging______obj_mut_dyn_Any____to__bool__{}", RUSTC_COMMIT);
                    type IsDraggingFn = unsafe extern "Rust" fn(&mut dyn Any) -> bool;
                    let getter_fn = {
                        let lib = self.loaded_libs.get(lib_name).ok_or("library not loaded")?;
                        let getter: Symbol<IsDraggingFn> = unsafe { lib.get(symbol_name.as_bytes())? };
                        *getter
                    };
                    let obj = self.get_object_mut(handle).ok_or("object not found")?;
                    let result = unsafe { getter_fn(obj) };
                    return Ok(Box::new(result));
                }
                "get_selected_handle" if args.is_empty() => {
                    let symbol_name = format!("WindowManager__get_selected_handle______obj_mut_dyn_Any____to__i64__{}", RUSTC_COMMIT);
                    type GetSelectedFn = unsafe extern "Rust" fn(&mut dyn Any) -> i64;
                    let getter_fn = {
                        let lib = self.loaded_libs.get(lib_name).ok_or("library not loaded")?;
                        let getter: Symbol<GetSelectedFn> = unsafe { lib.get(symbol_name.as_bytes())? };
                        *getter
                    };
                    let obj = self.get_object_mut(handle).ok_or("object not found")?;
                    let result = unsafe { getter_fn(obj) };
                    return Ok(Box::new(result));
                }
                "add_rect" if args.len() == 1 => {
                    let rect = *args[0].downcast_ref::<ObjectHandle>().ok_or("arg 0 not ObjectHandle")?;
                    let symbol_name = format!("WindowManager__add_rect______obj_mut_dyn_Any____rect__ObjectHandle____to__unit__{}", RUSTC_COMMIT);
                    type AddRectFn = unsafe extern "Rust" fn(&mut dyn Any, ObjectHandle);
                    let add_fn = {
                        let lib = self.loaded_libs.get(lib_name).ok_or("library not loaded")?;
                        let adder: Symbol<AddRectFn> = unsafe { lib.get(symbol_name.as_bytes())? };
                        *adder
                    };
                    let obj = self.get_object_mut(handle).ok_or("object not found")?;
                    unsafe { add_fn(obj, rect) };
                    return Ok(Box::new(()));
                }
                "start_dragging" if args.len() == 1 => {
                    let rect = *args[0].downcast_ref::<ObjectHandle>().ok_or("arg 0 not ObjectHandle")?;
                    let symbol_name = format!("WindowManager__start_dragging______obj_mut_dyn_Any____rect__ObjectHandle____to__unit__{}", RUSTC_COMMIT);
                    type StartDragFn = unsafe extern "Rust" fn(&mut dyn Any, ObjectHandle);
                    let start_fn = {
                        let lib = self.loaded_libs.get(lib_name).ok_or("library not loaded")?;
                        let starter: Symbol<StartDragFn> = unsafe { lib.get(symbol_name.as_bytes())? };
                        *starter
                    };
                    let obj = self.get_object_mut(handle).ok_or("object not found")?;
                    unsafe { start_fn(obj, rect) };
                    return Ok(Box::new(()));
                }
                "set_drag_offset" if args.len() == 2 => {
                    let x = *args[0].downcast_ref::<f64>().ok_or("arg 0 not f64")?;
                    let y = *args[1].downcast_ref::<f64>().ok_or("arg 1 not f64")?;
                    let symbol_name = format!("WindowManager__set_drag_offset______obj_mut_dyn_Any____x__f64____y__f64____to__unit__{}", RUSTC_COMMIT);
                    type SetOffsetFn = unsafe extern "Rust" fn(&mut dyn Any, f64, f64);
                    let set_fn = {
                        let lib = self.loaded_libs.get(lib_name).ok_or("library not loaded")?;
                        let setter: Symbol<SetOffsetFn> = unsafe { lib.get(symbol_name.as_bytes())? };
                        *setter
                    };
                    let obj = self.get_object_mut(handle).ok_or("object not found")?;
                    unsafe { set_fn(obj, x, y) };
                    return Ok(Box::new(()));
                }
                "clear_selection" | "stop_dragging" if args.is_empty() => {
                    let symbol_name = format!("WindowManager__{}______obj_mut_dyn_Any____to__unit__{}", method, RUSTC_COMMIT);
                    type VoidFn = unsafe extern "Rust" fn(&mut dyn Any);
                    let void_fn = {
                        let lib = self.loaded_libs.get(lib_name).ok_or("library not loaded")?;
                        let voider: Symbol<VoidFn> = unsafe { lib.get(symbol_name.as_bytes())? };
                        *voider
                    };
                    let obj = self.get_object_mut(handle).ok_or("object not found")?;
                    unsafe { void_fn(obj) };
                    return Ok(Box::new(()));
                }
                _ => {}
            }
        }
        
        // For now, just handle the move_by case for Rect
        if method == "move_by" && args.len() == 2 {
            let dx = *args[0].downcast_ref::<f64>().ok_or("arg 0 not f64")?;
            let dy = *args[1].downcast_ref::<f64>().ok_or("arg 1 not f64")?;

            // Use signature-encoded symbol name
            let symbol_name =
                format!("{}__{}______obj_mut_dyn_Any____dx__f64____dy__f64____to__unit__{}", type_name, method, RUSTC_COMMIT);

            type MoveFn = unsafe extern "Rust" fn(&mut dyn Any, f64, f64);
            let mover_fn = {
                let lib = self.loaded_libs.get(lib_name).ok_or("library not loaded")?;
                let mover: Symbol<MoveFn> = unsafe { lib.get(symbol_name.as_bytes())? };
                *mover
            };

            let obj = self.get_object_mut(handle).ok_or("object not found")?;

            unsafe { mover_fn(obj, dx, dy) };
            Ok(Box::new(()))
        } else {
            Err("unsupported method".into())
        }
    }
}

/// Macro for ergonomic direct calls
#[macro_export]
macro_rules! direct_call {
    // Rect field getters - return f64 directly
    ($runtime:expr, $handle:expr, Rect, x()) => {{
        $runtime.call_getter::<f64>($handle, "Rect", "librect", "x")
    }};
    ($runtime:expr, $handle:expr, Rect, y()) => {{
        $runtime.call_getter::<f64>($handle, "Rect", "librect", "y")
    }};
    ($runtime:expr, $handle:expr, Rect, width()) => {{
        $runtime.call_getter::<f64>($handle, "Rect", "librect", "width")
    }};
    ($runtime:expr, $handle:expr, Rect, height()) => {{
        $runtime.call_getter::<f64>($handle, "Rect", "librect", "height")
    }};
    
    // WindowManager field getters - return f64 directly
    ($runtime:expr, $handle:expr, WindowManager, drag_offset_x()) => {{
        $runtime.call_getter::<f64>($handle, "WindowManager", "libWindowManager", "drag_offset_x")
    }};
    ($runtime:expr, $handle:expr, WindowManager, drag_offset_y()) => {{
        $runtime.call_getter::<f64>($handle, "WindowManager", "libWindowManager", "drag_offset_y")
    }};
    
    // WindowManager methods that return i64
    ($runtime:expr, $handle:expr, WindowManager, get_rects_count()) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![];
        $runtime.call_method($handle, "WindowManager", "libWindowManager", "get_rects_count", args)
            .and_then(|r| r.downcast::<i64>()
                .map(|b| *b)
                .map_err(|_| "Failed to downcast get_rects_count result".into()))
    }};
    ($runtime:expr, $handle:expr, WindowManager, get_rect_at($index:expr)) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![Box::new($index)];
        $runtime.call_method($handle, "WindowManager", "libWindowManager", "get_rect_at", args)
            .and_then(|r| r.downcast::<i64>()
                .map(|b| *b)
                .map_err(|_| "Failed to downcast get_rect_at result".into()))
    }};
    ($runtime:expr, $handle:expr, WindowManager, get_selected_handle()) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![];
        $runtime.call_method($handle, "WindowManager", "libWindowManager", "get_selected_handle", args)
            .and_then(|r| r.downcast::<i64>()
                .map(|b| *b)
                .map_err(|_| "Failed to downcast get_selected_handle result".into()))
    }};
    
    // WindowManager methods that return bool
    ($runtime:expr, $handle:expr, WindowManager, is_dragging()) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![];
        $runtime.call_method($handle, "WindowManager", "libWindowManager", "is_dragging", args)
            .and_then(|r| r.downcast::<bool>()
                .map(|b| *b)
                .map_err(|_| "Failed to downcast is_dragging result".into()))
    }};
    
    // WindowManager void methods
    ($runtime:expr, $handle:expr, WindowManager, clear_selection()) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![];
        $runtime.call_method($handle, "WindowManager", "libWindowManager", "clear_selection", args)
    }};
    ($runtime:expr, $handle:expr, WindowManager, stop_dragging()) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![];
        $runtime.call_method($handle, "WindowManager", "libWindowManager", "stop_dragging", args)
    }};
    
    // WindowManager methods with arguments
    ($runtime:expr, $handle:expr, WindowManager, add_rect($rect:expr)) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![Box::new($rect)];
        $runtime.call_method($handle, "WindowManager", "libWindowManager", "add_rect", args)
    }};
    ($runtime:expr, $handle:expr, WindowManager, start_dragging($rect:expr)) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![Box::new($rect)];
        $runtime.call_method($handle, "WindowManager", "libWindowManager", "start_dragging", args)
    }};
    ($runtime:expr, $handle:expr, WindowManager, set_drag_offset($x:expr, $y:expr)) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![Box::new($x), Box::new($y)];
        $runtime.call_method($handle, "WindowManager", "libWindowManager", "set_drag_offset", args)
    }};

    // Rect setters
    ($runtime:expr, $handle:expr, Rect, set_x($value:expr)) => {{
        $runtime.call_setter($handle, "Rect", "librect", "set_x", $value)
    }};
    ($runtime:expr, $handle:expr, Rect, set_y($value:expr)) => {{
        $runtime.call_setter($handle, "Rect", "librect", "set_y", $value)
    }};
    ($runtime:expr, $handle:expr, Rect, set_width($value:expr)) => {{
        $runtime.call_setter($handle, "Rect", "librect", "set_width", $value)
    }};
    ($runtime:expr, $handle:expr, Rect, set_height($value:expr)) => {{
        $runtime.call_setter($handle, "Rect", "librect", "set_height", $value)
    }};
    
    // Rect methods
    ($runtime:expr, $handle:expr, Rect, move_by($dx:expr, $dy:expr)) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![Box::new($dx), Box::new($dy)];
        $runtime.call_method($handle, "Rect", "librect", "move_by", args)
    }};
}