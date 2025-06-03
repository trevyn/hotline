use hotline::{HotlineObject, LibraryRegistry, ObjectHandle};
use std::any::Any;
use std::sync::{Arc, Mutex};

pub const RUSTC_COMMIT: &str = env!("RUSTC_COMMIT_HASH");

pub struct DirectRuntime {
    library_registry: LibraryRegistry,
}

impl DirectRuntime {
    pub fn new() -> Self {
        Self { library_registry: LibraryRegistry::new() }
    }

    fn type_name_for_symbol<T: 'static>() -> &'static str {
        // Always use fully qualified type names for unambiguous type safety
        std::any::type_name::<T>()
    }

    pub fn register(&mut self, obj: Box<dyn HotlineObject>) -> ObjectHandle {
        Arc::new(Mutex::new(obj))
    }

    pub fn get_object<'a>(
        &self,
        handle: &'a ObjectHandle,
    ) -> Option<std::sync::MutexGuard<'a, Box<dyn HotlineObject>>> {
        handle.lock().ok()
    }

    pub fn get_object_mut<'a>(
        &mut self,
        handle: &'a ObjectHandle,
    ) -> Option<std::sync::MutexGuard<'a, Box<dyn HotlineObject>>> {
        handle.lock().ok()
    }

    pub fn hot_reload(&mut self, lib_path: &str, type_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        let lib_name = self.library_registry.load(lib_path)?;

        // Explicitly initialize the library with the registry
        let init_symbol = format!("{}__init__registry__{}", type_name, RUSTC_COMMIT);
        self.library_registry.with_symbol::<unsafe extern "C" fn(*const LibraryRegistry), _, _>(
            &lib_name,
            &init_symbol,
            |symbol| {
                unsafe { (**symbol)(&self.library_registry as *const LibraryRegistry) };
            },
        )?;

        Ok(())
    }

    // Create object from loaded library
    pub fn create_from_lib(
        &mut self,
        lib_name: &str,
        type_name: &str,
    ) -> Result<ObjectHandle, Box<dyn std::error::Error>> {
        let obj = self.library_registry.call_constructor(lib_name, type_name, RUSTC_COMMIT)?;
        let handle = self.register(obj);
        Ok(handle)
    }

    pub fn library_registry(&self) -> &LibraryRegistry {
        &self.library_registry
    }

    // Helper to call a symbol and get result
    fn call_symbol<T, R, F>(&self, lib_name: &str, symbol_name: &str, f: F) -> Result<R, Box<dyn std::error::Error>>
    where
        T: 'static,
        F: FnOnce(&hotline::libloading::Symbol<T>) -> R,
    {
        self.library_registry.with_symbol::<T, _, _>(lib_name, symbol_name, f)
    }

    // Direct method calls
    pub fn call_getter<T>(
        &self,
        handle: &ObjectHandle,
        type_name: &str,
        lib_name: &str,
        method: &str,
    ) -> Result<T, Box<dyn std::error::Error>>
    where
        T: Clone + 'static,
    {
        let obj_guard = self.get_object(handle).ok_or("object not found")?;
        let obj_any = obj_guard.as_any();

        // TODO: Ideally we'd compute this once and cache
        let type_str = Self::type_name_for_symbol::<T>();
        let symbol_name =
            format!("{}__get_{}____obj_ref_dyn_Any__to__{}__{}", type_name, method, type_str, RUSTC_COMMIT);

        println!("Looking for getter symbol: {}", symbol_name);
        type GetterFn<T> = unsafe extern "Rust" fn(&dyn Any) -> T;

        self.library_registry
            .with_symbol::<GetterFn<T>, _, _>(lib_name, &symbol_name, |getter_fn| unsafe { (**getter_fn)(obj_any) })
    }

    pub fn call_setter<T>(
        &mut self,
        handle: &ObjectHandle,
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

        let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
        let obj = obj_guard.as_any_mut();

        println!("Calling setter {}::{} with value {:?}", type_name, method, value);

        self.library_registry
            .with_symbol::<SetterFn<T>, _, _>(lib_name, &symbol_name, |setter_fn| {
                unsafe { (**setter_fn)(obj, value) };
            })
            .map_err(|e| {
                eprintln!("Failed to find symbol {}: {:?}", symbol_name, e);
                format!("Symbol not found: {}", symbol_name).into()
            })
    }

    pub fn call_method(
        &mut self,
        handle: &ObjectHandle,
        type_name: &str,
        lib_name: &str,
        method: &str,
        args: Vec<Box<dyn Any>>,
    ) -> Result<Box<dyn Any>, Box<dyn std::error::Error>> {
        // Handle WindowManager methods
        if type_name == "WindowManager" {
            match method {
                "get_rects_count" if args.is_empty() => {
                    let symbol_name =
                        format!("WindowManager__get_rects_count______obj_mut_dyn_Any____to__i64__{}", RUSTC_COMMIT);
                    type GetCountFn = unsafe extern "Rust" fn(&mut dyn Any) -> i64;
                    let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
                    let obj = obj_guard.as_any_mut();
                    return self
                        .library_registry
                        .with_symbol::<GetCountFn, _, _>(lib_name, &symbol_name, |getter_fn| {
                            let result = unsafe { (**getter_fn)(obj) };
                            Box::new(result)
                        })
                        .map(|b| Box::new(b) as Box<dyn Any>);
                }
                "get_rect_at" if args.len() == 1 => {
                    let index = *args[0].downcast_ref::<i64>().ok_or("arg 0 not i64")?;
                    let symbol_name = format!(
                        "WindowManager__get_rect_at______obj_mut_dyn_Any____index__i64____to__Option_lt_ObjectHandle_gt__{}",
                        RUSTC_COMMIT
                    );
                    type GetAtFn = unsafe extern "Rust" fn(&mut dyn Any, i64) -> Option<ObjectHandle>;
                    let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
                    let obj = obj_guard.as_any_mut();
                    return self.library_registry.with_symbol::<GetAtFn, _, _>(lib_name, &symbol_name, |getter_fn| {
                        let result = unsafe { (*getter_fn)(obj, index) };
                        Box::new(result) as Box<dyn Any>
                    });
                }
                "is_dragging" if args.is_empty() => {
                    let symbol_name =
                        format!("WindowManager__is_dragging______obj_mut_dyn_Any____to__bool__{}", RUSTC_COMMIT);
                    type IsDraggingFn = unsafe extern "Rust" fn(&mut dyn Any) -> bool;
                    let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
                    let obj = obj_guard.as_any_mut();
                    return self.call_symbol::<IsDraggingFn, _, _>(lib_name, &symbol_name, |getter_fn| {
                        let result = unsafe { (**getter_fn)(obj) };
                        Box::new(result) as Box<dyn Any>
                    });
                }
                "get_selected_handle" if args.is_empty() => {
                    let symbol_name = format!(
                        "WindowManager__get_selected_handle______obj_mut_dyn_Any____to__Option_lt_ObjectHandle_gt__{}",
                        RUSTC_COMMIT
                    );
                    type GetSelectedFn = unsafe extern "Rust" fn(&mut dyn Any) -> Option<ObjectHandle>;
                    let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
                    let obj = obj_guard.as_any_mut();
                    return self.call_symbol::<GetSelectedFn, _, _>(lib_name, &symbol_name, |getter_fn| {
                        let result = unsafe { (**getter_fn)(obj) };
                        Box::new(result) as Box<dyn Any>
                    });
                }
                "add_rect" if args.len() == 1 => {
                    let rect = args[0].downcast_ref::<ObjectHandle>().ok_or("arg 0 not ObjectHandle")?.clone();
                    let symbol_name = format!(
                        "WindowManager__add_rect______obj_mut_dyn_Any____rect__ObjectHandle____to__unit__{}",
                        RUSTC_COMMIT
                    );
                    type AddRectFn = unsafe extern "Rust" fn(&mut dyn Any, ObjectHandle);
                    let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
                    let obj = obj_guard.as_any_mut();
                    return self.call_symbol::<AddRectFn, _, _>(lib_name, &symbol_name, |add_fn| {
                        unsafe { (**add_fn)(obj, rect) };
                        Box::new(()) as Box<dyn Any>
                    });
                }
                "start_dragging" if args.len() == 1 => {
                    let rect = args[0].downcast_ref::<ObjectHandle>().ok_or("arg 0 not ObjectHandle")?.clone();
                    let symbol_name = format!(
                        "WindowManager__start_dragging______obj_mut_dyn_Any____rect__ObjectHandle____to__unit__{}",
                        RUSTC_COMMIT
                    );
                    type StartDragFn = unsafe extern "Rust" fn(&mut dyn Any, ObjectHandle);
                    let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
                    let obj = obj_guard.as_any_mut();
                    return self.call_symbol::<StartDragFn, _, _>(lib_name, &symbol_name, |start_fn| {
                        unsafe { (**start_fn)(obj, rect) };
                        Box::new(()) as Box<dyn Any>
                    });
                }
                "set_drag_offset" if args.len() == 2 => {
                    let x = *args[0].downcast_ref::<f64>().ok_or("arg 0 not f64")?;
                    let y = *args[1].downcast_ref::<f64>().ok_or("arg 1 not f64")?;
                    let symbol_name = format!(
                        "WindowManager__set_drag_offset______obj_mut_dyn_Any____x__f64____y__f64____to__unit__{}",
                        RUSTC_COMMIT
                    );
                    type SetOffsetFn = unsafe extern "Rust" fn(&mut dyn Any, f64, f64);
                    let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
                    let obj = obj_guard.as_any_mut();
                    return self.call_symbol::<SetOffsetFn, _, _>(lib_name, &symbol_name, |set_fn| {
                        unsafe { (**set_fn)(obj, x, y) };
                        Box::new(()) as Box<dyn Any>
                    });
                }
                "handle_mouse_down" if args.len() == 2 => {
                    let x = *args[0].downcast_ref::<f64>().ok_or("arg 0 not f64")?;
                    let y = *args[1].downcast_ref::<f64>().ok_or("arg 1 not f64")?;
                    let symbol_name = format!(
                        "WindowManager__handle_mouse_down______obj_mut_dyn_Any____x__f64____y__f64____to__unit__{}",
                        RUSTC_COMMIT
                    );
                    type MouseDownFn = unsafe extern "Rust" fn(&mut dyn Any, f64, f64);
                    let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
                    let obj = obj_guard.as_any_mut();
                    return self.call_symbol::<MouseDownFn, _, _>(lib_name, &symbol_name, |mouse_fn| {
                        unsafe { (**mouse_fn)(obj, x, y) };
                        Box::new(()) as Box<dyn Any>
                    });
                }
                "handle_mouse_up" if args.len() == 2 => {
                    let x = *args[0].downcast_ref::<f64>().ok_or("arg 0 not f64")?;
                    let y = *args[1].downcast_ref::<f64>().ok_or("arg 1 not f64")?;
                    let symbol_name = format!(
                        "WindowManager__handle_mouse_up______obj_mut_dyn_Any____x__f64____y__f64____to__unit__{}",
                        RUSTC_COMMIT
                    );
                    type MouseUpFn = unsafe extern "Rust" fn(&mut dyn Any, f64, f64);
                    let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
                    let obj = obj_guard.as_any_mut();
                    return self.call_symbol::<MouseUpFn, _, _>(lib_name, &symbol_name, |mouse_fn| {
                        unsafe { (**mouse_fn)(obj, x, y) };
                        Box::new(()) as Box<dyn Any>
                    });
                }
                "handle_mouse_motion" if args.len() == 2 => {
                    let x = *args[0].downcast_ref::<f64>().ok_or("arg 0 not f64")?;
                    let y = *args[1].downcast_ref::<f64>().ok_or("arg 1 not f64")?;
                    let symbol_name = format!(
                        "WindowManager__handle_mouse_motion______obj_mut_dyn_Any____x__f64____y__f64____to__unit__{}",
                        RUSTC_COMMIT
                    );
                    type MouseMotionFn = unsafe extern "Rust" fn(&mut dyn Any, f64, f64);
                    let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
                    let obj = obj_guard.as_any_mut();
                    return self.call_symbol::<MouseMotionFn, _, _>(lib_name, &symbol_name, |mouse_fn| {
                        unsafe { (**mouse_fn)(obj, x, y) };
                        Box::new(()) as Box<dyn Any>
                    });
                }
                "render" if args.len() == 4 => {
                    // Special handling for render which takes a buffer slice
                    // This is a hack - we'll need to pass the buffer differently
                    return Err("render method needs special handling".into());
                }
                "clear_selection" | "stop_dragging" if args.is_empty() => {
                    let symbol_name =
                        format!("WindowManager__{}______obj_mut_dyn_Any____to__unit__{}", method, RUSTC_COMMIT);
                    type VoidFn = unsafe extern "Rust" fn(&mut dyn Any);
                    let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
                    let obj = obj_guard.as_any_mut();
                    return self.call_symbol::<VoidFn, _, _>(lib_name, &symbol_name, |void_fn| {
                        unsafe { (**void_fn)(obj) };
                        Box::new(()) as Box<dyn Any>
                    });
                }
                _ => {}
            }
        }

        // Handle Rect methods
        if type_name == "Rect" {
            match method {
                "initialize" if args.len() == 4 => {
                    let x = *args[0].downcast_ref::<f64>().ok_or("arg 0 not f64")?;
                    let y = *args[1].downcast_ref::<f64>().ok_or("arg 1 not f64")?;
                    let width = *args[2].downcast_ref::<f64>().ok_or("arg 2 not f64")?;
                    let height = *args[3].downcast_ref::<f64>().ok_or("arg 3 not f64")?;
                    let symbol_name = format!(
                        "Rect__initialize______obj_mut_dyn_Any____x__f64____y__f64____width__f64____height__f64____to__unit__{}",
                        RUSTC_COMMIT
                    );
                    type InitFn = unsafe extern "Rust" fn(&mut dyn Any, f64, f64, f64, f64);
                    let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
                    let obj = obj_guard.as_any_mut();
                    return self.call_symbol::<InitFn, _, _>(lib_name, &symbol_name, |init_fn| {
                        unsafe { (**init_fn)(obj, x, y, width, height) };
                        Box::new(()) as Box<dyn Any>
                    });
                }
                "contains_point" if args.len() == 2 => {
                    let x = *args[0].downcast_ref::<f64>().ok_or("arg 0 not f64")?;
                    let y = *args[1].downcast_ref::<f64>().ok_or("arg 1 not f64")?;
                    let symbol_name = format!(
                        "Rect__contains_point______obj_mut_dyn_Any____point_x__f64____point_y__f64____to__bool__{}",
                        RUSTC_COMMIT
                    );
                    type ContainsFn = unsafe extern "Rust" fn(&mut dyn Any, f64, f64) -> bool;
                    let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
                    let obj = obj_guard.as_any_mut();
                    return self.call_symbol::<ContainsFn, _, _>(lib_name, &symbol_name, |contains_fn| {
                        let result = unsafe { (**contains_fn)(obj, x, y) };
                        Box::new(result) as Box<dyn Any>
                    });
                }
                "position" if args.is_empty() => {
                    let symbol_name =
                        format!("Rect__position______obj_mut_dyn_Any____to__tuple_f64_comma_f64__{}", RUSTC_COMMIT);
                    type GetPosFn = unsafe extern "Rust" fn(&mut dyn Any) -> (f64, f64);
                    let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
                    let obj = obj_guard.as_any_mut();
                    return self.call_symbol::<GetPosFn, _, _>(lib_name, &symbol_name, |getpos_fn| {
                        let result = unsafe { (**getpos_fn)(obj) };
                        Box::new(result) as Box<dyn Any>
                    });
                }
                "bounds" if args.is_empty() => {
                    let symbol_name = format!(
                        "Rect__bounds______obj_mut_dyn_Any____to__tuple_f64_comma_f64_comma_f64_comma_f64__{}",
                        RUSTC_COMMIT
                    );
                    type GetBoundsFn = unsafe extern "Rust" fn(&mut dyn Any) -> (f64, f64, f64, f64);
                    let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
                    let obj = obj_guard.as_any_mut();
                    return self.call_symbol::<GetBoundsFn, _, _>(lib_name, &symbol_name, |getbounds_fn| {
                        let result = unsafe { (**getbounds_fn)(obj) };
                        Box::new(result) as Box<dyn Any>
                    });
                }
                _ => {}
            }
        }

        // Handle move_by for any type
        if method == "move_by" && args.len() == 2 {
            let dx = *args[0].downcast_ref::<f64>().ok_or("arg 0 not f64")?;
            let dy = *args[1].downcast_ref::<f64>().ok_or("arg 1 not f64")?;

            // Use signature-encoded symbol name
            let symbol_name = format!(
                "{}__{}______obj_mut_dyn_Any____dx__f64____dy__f64____to__unit__{}",
                type_name, method, RUSTC_COMMIT
            );

            type MoveFn = unsafe extern "Rust" fn(&mut dyn Any, f64, f64);
            let mut obj_guard = self.get_object_mut(handle).ok_or("object not found")?;
            let obj = obj_guard.as_any_mut();

            self.call_symbol::<MoveFn, _, _>(lib_name, &symbol_name, |mover_fn| {
                unsafe { (**mover_fn)(obj, dx, dy) };
                Box::new(()) as Box<dyn Any>
            })
        } else {
            Err("unsupported method".into())
        }
    }
}

/// Generic macro for direct calls - downcasting must be done by caller
#[macro_export]
macro_rules! direct_call {
    // Method call with no arguments
    ($runtime:expr, $handle:expr, $type:ident, $method:ident()) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![];
        let type_name = stringify!($type);
        let lib_name = concat!("lib", stringify!($type));
        let method_name = stringify!($method);
        $runtime.call_method($handle, type_name, lib_name, method_name, args)
    }};

    // Method call with arguments
    ($runtime:expr, $handle:expr, $type:ident, $method:ident($($arg:expr),*)) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![$(Box::new($arg)),*];
        let type_name = stringify!($type);
        let lib_name = concat!("lib", stringify!($type));
        let method_name = stringify!($method);
        $runtime.call_method($handle, type_name, lib_name, method_name, args)
    }};
}

// Keep old patterns for compatibility during transition
#[macro_export]
macro_rules! direct_call_old {
    // Rect field getters - return f64 directly
    ($runtime:expr, $handle:expr, Rect, x()) => {{ $runtime.call_getter::<f64>($handle, "Rect", "libRect", "x") }};
    ($runtime:expr, $handle:expr, Rect, y()) => {{ $runtime.call_getter::<f64>($handle, "Rect", "libRect", "y") }};
    ($runtime:expr, $handle:expr, Rect, width()) => {{ $runtime.call_getter::<f64>($handle, "Rect", "libRect", "width") }};
    ($runtime:expr, $handle:expr, Rect, height()) => {{ $runtime.call_getter::<f64>($handle, "Rect", "libRect", "height") }};

    // WindowManager field getters - return f64 directly
    ($runtime:expr, $handle:expr, WindowManager, drag_offset_x()) => {{ $runtime.call_getter::<f64>($handle, "WindowManager", "libWindowManager", "drag_offset_x") }};
    ($runtime:expr, $handle:expr, WindowManager, drag_offset_y()) => {{ $runtime.call_getter::<f64>($handle, "WindowManager", "libWindowManager", "drag_offset_y") }};

    // WindowManager methods that return i64
    ($runtime:expr, $handle:expr, WindowManager, get_rects_count()) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![];
        $runtime.call_method($handle, "WindowManager", "libWindowManager", "get_rects_count", args).and_then(|r| {
            r.downcast::<i64>().map(|b| *b).map_err(|_| "Failed to downcast get_rects_count result".into())
        })
    }};
    ($runtime:expr, $handle:expr, WindowManager, get_rect_at($index:expr)) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![Box::new($index)];
        $runtime
            .call_method($handle, "WindowManager", "libWindowManager", "get_rect_at", args)
            .and_then(|r| r.downcast::<i64>().map(|b| *b).map_err(|_| "Failed to downcast get_rect_at result".into()))
    }};
    ($runtime:expr, $handle:expr, WindowManager, get_selected_handle()) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![];
        $runtime.call_method($handle, "WindowManager", "libWindowManager", "get_selected_handle", args).and_then(|r| {
            r.downcast::<i64>().map(|b| *b).map_err(|_| "Failed to downcast get_selected_handle result".into())
        })
    }};

    // WindowManager methods that return bool
    ($runtime:expr, $handle:expr, WindowManager, is_dragging()) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![];
        $runtime
            .call_method($handle, "WindowManager", "libWindowManager", "is_dragging", args)
            .and_then(|r| r.downcast::<bool>().map(|b| *b).map_err(|_| "Failed to downcast is_dragging result".into()))
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
    ($runtime:expr, $handle:expr, WindowManager, start_drag_at($rect:expr, $mx:expr, $my:expr, $rx:expr, $ry:expr)) => {{
        let args: Vec<Box<dyn std::any::Any>> =
            vec![Box::new($rect), Box::new($mx), Box::new($my), Box::new($rx), Box::new($ry)];
        $runtime.call_method($handle, "WindowManager", "libWindowManager", "start_drag_at", args)
    }};
    ($runtime:expr, $handle:expr, WindowManager, get_drag_position($x:expr, $y:expr)) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![Box::new($x), Box::new($y)];
        $runtime.call_method($handle, "WindowManager", "libWindowManager", "get_drag_position", args).and_then(|r| {
            r.downcast::<Option<(f64, f64, ObjectHandle)>>()
                .map(|b| *b)
                .map_err(|_| "Failed to downcast get_drag_position result".into())
        })
    }};

    // Rect methods
    ($runtime:expr, $handle:expr, Rect, initialize($x:expr, $y:expr, $width:expr, $height:expr)) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![Box::new($x), Box::new($y), Box::new($width), Box::new($height)];
        $runtime.call_method($handle, "Rect", "libRect", "initialize", args)
    }};
    ($runtime:expr, $handle:expr, Rect, contains_point($x:expr, $y:expr)) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![Box::new($x), Box::new($y)];
        $runtime.call_method($handle, "Rect", "libRect", "contains_point", args).and_then(|r| {
            r.downcast::<bool>().map(|b| *b).map_err(|_| "Failed to downcast contains_point result".into())
        })
    }};
    ($runtime:expr, $handle:expr, Rect, get_position()) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![];
        $runtime.call_method($handle, "Rect", "libRect", "get_position", args).and_then(|r| {
            r.downcast::<(f64, f64)>().map(|b| *b).map_err(|_| "Failed to downcast get_position result".into())
        })
    }};
    ($runtime:expr, $handle:expr, Rect, get_bounds()) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![];
        $runtime.call_method($handle, "Rect", "libRect", "get_bounds", args).and_then(|r| {
            r.downcast::<(f64, f64, f64, f64)>().map(|b| *b).map_err(|_| "Failed to downcast get_bounds result".into())
        })
    }};
    ($runtime:expr, $handle:expr, Rect, move_by($dx:expr, $dy:expr)) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![Box::new($dx), Box::new($dy)];
        $runtime.call_method($handle, "Rect", "libRect", "move_by", args)
    }};
}
