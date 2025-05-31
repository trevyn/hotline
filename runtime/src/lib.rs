use hotline::{Message, Object, ObjectHandle, Value, Bounds, Deserialize};
use libloading::Library;
use std::collections::HashMap;

pub struct Runtime {
    objects: HashMap<ObjectHandle, (String, Box<dyn Object>)>, // (class_name, object)
    classes: HashMap<String, Box<dyn Fn() -> Box<dyn Object>>>,
    next_id: u64,
    loaded_libs: HashMap<String, Library>,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            objects: HashMap::new(),
            classes: HashMap::new(),
            next_id: 1,
            loaded_libs: HashMap::new(),
        }
    }

    pub fn send(&mut self, handle: ObjectHandle, selector: &str, args: Vec<Value>) -> Value {
        let msg = Message { selector: selector.to_string(), args };

        match self.objects.get_mut(&handle) {
            Some((_, obj)) => obj.receive(&msg),
            None => Value::Nil,
        }
    }

    pub fn send0(&mut self, handle: ObjectHandle, selector: &str) -> Value {
        self.send(handle, selector, vec![])
    }

    pub fn send1(&mut self, handle: ObjectHandle, selector: &str, arg: Value) -> Value {
        self.send(handle, selector, vec![arg])
    }

    pub fn create(&mut self, class: &str) -> Option<ObjectHandle> {
        let factory = self.classes.get(class)?;
        let obj = factory();
        let handle = ObjectHandle(self.next_id);
        self.next_id += 1;
        self.objects.insert(handle, (class.to_string(), obj));
        Some(handle)
    }

    pub fn hot_reload(&mut self, lib_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Unload old version
        if let Some(old_lib) = self.loaded_libs.remove(lib_path) {
            drop(old_lib);
        }

        // Load new version
        unsafe {
            let lib = Library::new(lib_path)?;

            // Get registration function
            let register_fn: libloading::Symbol<hotline::RegisterFn> =
                lib.get(b"register_objects")?;

            // Clear old class registrations
            self.classes.clear();

            // Re-register classes
            let classes_ptr =
                &mut self.classes as *mut HashMap<String, Box<dyn Fn() -> Box<dyn Object>>>;
            let mut registrar: Box<dyn FnMut(&str, Box<dyn Fn() -> Box<dyn Object>>)> =
                Box::new(move |name: &str, factory: Box<dyn Fn() -> Box<dyn Object>>| {
                    (*classes_ptr).insert(name.to_string(), factory);
                });

            register_fn(&mut registrar as *mut _ as *mut std::ffi::c_void);

            // Now update all existing objects with new implementations
            for (_handle, (class_name, old_obj)) in self.objects.iter_mut() {
                if let Some(factory) = self.classes.get(class_name) {
                    // Serialize state from old object
                    let state = old_obj.serialize();
                    
                    // Create new object and restore state
                    let mut new_obj = factory();
                    new_obj.deserialize(&state);
                    
                    // Replace old object with new one
                    *old_obj = new_obj;
                }
            }

            self.loaded_libs.insert(lib_path.to_string(), lib);
        }

        Ok(())
    }

    pub fn register_static(&mut self, name: &str, factory: Box<dyn Fn() -> Box<dyn Object>>) {
        self.classes.insert(name.to_string(), factory);
    }
    
    pub fn with_object<F, R>(&self, handle: ObjectHandle, f: F) -> Option<R>
    where
        F: FnOnce(&dyn Object) -> R,
    {
        self.objects.get(&handle).map(|(_, obj)| f(obj.as_ref()))
    }
    
    pub fn render_object(&self, handle: ObjectHandle, buffer: &mut [u8], width: i64, height: i64, pitch: i64) {
        if let Some((class_name, obj)) = self.objects.get(&handle) {
            // Look for the render function for this class
            for lib in self.loaded_libs.values() {
                let render_fn_name = format!("render_{}", class_name.to_lowercase());
                unsafe {
                    // The render function takes &dyn Any
                    type RenderFn = fn(&dyn std::any::Any, &mut [u8], i64, i64, i64);
                    if let Ok(symbol) = lib.get::<RenderFn>(render_fn_name.as_bytes()) {
                        let render_fn = *symbol;
                        // Get the object as &dyn Any
                        let any_obj = obj.as_any();
                        render_fn(any_obj, buffer, width, height, pitch);
                        return;
                    }
                }
            }
        }
    }
    
    pub fn get_bounds(&mut self, handle: ObjectHandle) -> Option<Bounds> {
        // First try bounds as a method with dummy arg
        let bounds_value = self.send1(handle, "bounds:", Value::Int(0));
        if let Some(bounds) = Bounds::deserialize(&bounds_value) {
            return Some(bounds);
        }
        
        // Fallback to getter (for objects that expose bounds as a property)
        let bounds_value = self.send0(handle, "bounds");
        Bounds::deserialize(&bounds_value)
    }
    
    pub fn hit_test(&mut self, x: f64, y: f64) -> Option<ObjectHandle> {
        // Check objects in reverse order (top to bottom)
        let handles: Vec<_> = self.objects.keys().cloned().collect();
        for handle in handles.into_iter().rev() {
            if let Some(bounds) = self.get_bounds(handle) {
                if bounds.contains(x, y) {
                    return Some(handle);
                }
            }
        }
        None
    }
}

// Message macro - assumes 'runtime' is in scope
#[macro_export]
macro_rules! m {
    // Unary message: m![runtime, rect, x]
    ($runtime:expr, $obj:expr, $selector:ident) => {
        $runtime.send0($obj, stringify!($selector))
    };

    // Binary message: m![runtime, point, + otherPoint]
    ($runtime:expr, $obj:expr, $op:tt $arg:expr) => {
        $runtime.send($obj, stringify!($op), vec![hotline::to_value($arg)])
    };

    // Keyword message: m![runtime, rect, initWithX:10 y:20 width:100 height:50]
    ($runtime:expr, $obj:expr, $($part:ident : $arg:expr)*) => {{
        let selector = concat!($(stringify!($part), ":",)*);
        let args = vec![$(hotline::to_value($arg),)*];
        $runtime.send($obj, selector, args)
    }};
}
