use hotline::{Message, Object, ObjectHandle, Value};
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
