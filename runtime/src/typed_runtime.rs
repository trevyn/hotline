use hotline::{TypedObject, TypedMessage, TypedValue, ObjectHandle, MethodSignature};
use std::collections::HashMap;
use libloading::{Library, Symbol};

pub struct TypedRuntime {
    objects: HashMap<ObjectHandle, Box<dyn TypedObject>>,
    next_handle: u64,
    loaded_libs: HashMap<String, Library>,
}

impl TypedRuntime {
    pub fn new() -> Self {
        Self {
            objects: HashMap::new(),
            next_handle: 1,
            loaded_libs: HashMap::new(),
        }
    }
    
    pub fn register(&mut self, obj: Box<dyn TypedObject>) -> ObjectHandle {
        let handle = ObjectHandle(self.next_handle);
        self.next_handle += 1;
        self.objects.insert(handle, obj);
        handle
    }
    
    pub fn send(&mut self, target: ObjectHandle, msg: TypedMessage) -> Result<TypedValue, String> {
        let obj = self.objects.get_mut(&target)
            .ok_or_else(|| format!("no object with handle {:?}", target))?;
        
        // validate message against signatures
        let signatures = obj.signatures();
        let sig = signatures.iter()
            .find(|s| s.selector == msg.selector)
            .ok_or_else(|| format!("object does not respond to '{}'", msg.selector))?
            .clone();
        
        // check arg count
        if msg.args.len() != sig.arg_types.len() {
            return Err(format!(
                "'{}' expects {} args, got {}",
                msg.selector, sig.arg_types.len(), msg.args.len()
            ));
        }
        
        // check arg types
        for (i, (arg, expected_type)) in msg.args.iter().zip(&sig.arg_types).enumerate() {
            if &arg.type_id() != expected_type {
                return Err(format!(
                    "'{}' arg {} type mismatch",
                    msg.selector, i
                ));
            }
        }
        
        // dispatch
        let result = obj.receive_typed(&msg)?;
        
        // validate return type
        if result.type_id() != sig.return_type {
            return Err(format!(
                "'{}' return type mismatch",
                msg.selector
            ));
        }
        
        Ok(result)
    }
    
    pub fn get_object(&self, handle: ObjectHandle) -> Option<&dyn TypedObject> {
        self.objects.get(&handle).map(|b| &**b)
    }
    
    pub fn get_object_mut(&mut self, handle: ObjectHandle) -> Option<&mut dyn TypedObject> {
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
    
    pub fn create_from_lib(&mut self, lib_name: &str, creator_fn: &str) -> Option<ObjectHandle> {
        let lib = self.loaded_libs.get(lib_name)?;
        
        unsafe {
            // Use Rust ABI to preserve fat pointers for trait objects
            type CreatorFn = unsafe extern "Rust" fn() -> Box<dyn TypedObject>;
            let creator: Symbol<CreatorFn> = 
                lib.get(creator_fn.as_bytes()).ok()?;
            let obj = creator();
            Some(self.register(obj))
        }
    }
}

/// macro for ergonomic message sends
#[macro_export]
macro_rules! typed_send {
    ($runtime:expr, $target:expr, $selector:ident($($arg:expr),*)) => {{
        let msg = TypedMessage {
            selector: stringify!($selector).to_string(),
            args: vec![$(TypedValue::new($arg)),*],
        };
        $runtime.send($target, msg)
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use hotline::typed_methods;
    
    #[derive(Clone)]
    struct Counter {
        value: i64,
    }
    
    typed_methods! {
        Counter {
            fn increment(&mut self) {
                self.value += 1;
            }
            
            fn add(&mut self, n: i64) {
                self.value += n;
            }
            
            fn get(&mut self) -> i64 {
                self.value
            }
        }
    }
    
    #[test]
    fn test_typed_dispatch() {
        let mut runtime = TypedRuntime::new();
        let handle = runtime.register(Box::new(Counter { value: 0 }));
        
        // increment
        typed_send!(runtime, handle, increment()).unwrap();
        
        // add
        typed_send!(runtime, handle, add(5i64)).unwrap();
        
        // get
        let result = typed_send!(runtime, handle, get()).unwrap();
        assert_eq!(*result.get::<i64>().unwrap(), 6);
        
        // type error
        let msg = TypedMessage {
            selector: "add".to_string(),
            args: vec![TypedValue::new("not a number")],
        };
        assert!(runtime.send(handle, msg).is_err());
    }
}