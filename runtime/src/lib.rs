use hotline::ObjectHandle;
use libloading::{Library, Symbol};
use std::any::Any;
use std::collections::HashMap;

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

    pub fn create_from_lib(
        &mut self,
        lib_name: &str,
        type_name: &str,
    ) -> Result<ObjectHandle, Box<dyn std::error::Error>> {
        let lib = self.loaded_libs.get(lib_name).ok_or("library not loaded")?;

        // Call constructor with signature-encoded name
        let constructor_symbol = format!("{}__new____to__Box_lt_dyn_Any_gt", type_name);
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

        // Use signature-encoded symbol name
        let return_type = Self::type_name_for_symbol::<T>();
        let symbol_name =
            format!("{}__get_{}____obj_ref_dyn_Any__to__{}", type_name, method, return_type);
        type GetterFn<T> = unsafe extern "Rust" fn(&dyn Any) -> T;
        let getter: Symbol<GetterFn<T>> = unsafe { lib.get(symbol_name.as_bytes())? };

        let result = unsafe { getter(obj) };
        Ok(result)
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
            "{}__set_{}____obj_mut_dyn_Any__{}_{}__{}",
            type_name, field_name, field_name, value_type, "to__unit"
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
        // For now, just handle the move_by case
        if method == "move_by" && args.len() == 2 {
            let dx = *args[0].downcast_ref::<f64>().ok_or("arg 0 not f64")?;
            let dy = *args[1].downcast_ref::<f64>().ok_or("arg 1 not f64")?;

            // Use signature-encoded symbol name
            let symbol_name =
                format!("{}__{}____obj_mut_dyn_Any__dx_f64__dy_f64__to__unit", type_name, method);

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
    // Getter
    ($runtime:expr, $handle:expr, $type:ident, $method:ident()) => {{
        $runtime.call_getter::<_>($handle, stringify!($type), "librect", stringify!($method))
    }};

    // Setter
    ($runtime:expr, $handle:expr, $type:ident, $method:ident($value:expr)) => {{
        $runtime.call_setter($handle, stringify!($type), "librect", stringify!($method), $value)
    }};

    // Method with args
    ($runtime:expr, $handle:expr, $type:ident, $method:ident($($arg:expr),*)) => {{
        let args: Vec<Box<dyn std::any::Any>> = vec![$(Box::new($arg)),*];
        $runtime.call_method($handle, stringify!($type), "librect", stringify!($method), args)
    }};
}
