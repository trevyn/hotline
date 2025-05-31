use std::collections::HashMap;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectHandle(pub u64);

#[repr(C)]
#[derive(Debug, Clone)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Symbol(String),
    Object(ObjectHandle),
    Array(Vec<Value>),
    Dict(HashMap<String, Value>),
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Message {
    pub selector: String,
    pub args: Vec<Value>,
}

pub trait Object: Send + Sync {
    fn receive(&mut self, msg: &Message) -> Value;
    
    // Serialization support for hot-reloading
    fn serialize(&self) -> Value {
        Value::Nil // Default implementation
    }
    
    fn deserialize(&mut self, _state: &Value) {
        // Default implementation does nothing
    }
}

// Conversion helpers
impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Value::Int(v as i64)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Value::Int(v)
    }
}

impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Value::Float(v as f64)
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Value::Float(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Value::String(v.to_string())
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Value::String(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Value::Bool(v)
    }
}

impl From<ObjectHandle> for Value {
    fn from(v: ObjectHandle) -> Self {
        Value::Object(v)
    }
}

pub fn to_value<T: Into<Value>>(v: T) -> Value {
    v.into()
}

// Serialization helpers
pub trait Serialize {
    fn serialize(&self) -> Value;
}

pub trait Deserialize {
    fn deserialize(value: &Value) -> Option<Self> where Self: Sized;
}

// Implement for common types
impl Serialize for f64 {
    fn serialize(&self) -> Value {
        Value::Float(*self)
    }
}

impl Deserialize for f64 {
    fn deserialize(value: &Value) -> Option<Self> {
        match value {
            Value::Float(f) => Some(*f),
            Value::Int(i) => Some(*i as f64),
            _ => None,
        }
    }
}

impl Serialize for i64 {
    fn serialize(&self) -> Value {
        Value::Int(*self)
    }
}

impl Deserialize for i64 {
    fn deserialize(value: &Value) -> Option<Self> {
        match value {
            Value::Int(i) => Some(*i),
            _ => None,
        }
    }
}

impl Serialize for String {
    fn serialize(&self) -> Value {
        Value::String(self.clone())
    }
}

impl Deserialize for String {
    fn deserialize(value: &Value) -> Option<Self> {
        match value {
            Value::String(s) => Some(s.clone()),
            _ => None,
        }
    }
}

impl Serialize for bool {
    fn serialize(&self) -> Value {
        Value::Bool(*self)
    }
}

impl Deserialize for bool {
    fn deserialize(value: &Value) -> Option<Self> {
        match value {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

// Helper macro for creating dict Values
#[macro_export]
macro_rules! dict {
    ($($key:expr => $value:expr),* $(,)?) => {{
        let mut map = std::collections::HashMap::new();
        $(
            map.insert($key.to_string(), $value);
        )*
        Value::Dict(map)
    }};
}

// Function type for object registration - using opaque pointer for FFI safety
pub type RegisterFn = unsafe extern "C" fn(*mut std::ffi::c_void);

// Macro for defining objects with automatic boilerplate
#[macro_export]
macro_rules! object {
    (
        $name:ident {
            $(
                $field:ident: $type:ty
            ),* $(,)?
        }
    ) => {
        #[derive(Default)]
        pub struct $name {
            $(
                pub $field: $type,
            )*
        }
        
        impl $name {
            fn auto_receive(&mut self, msg: &Message) -> Option<Value> {
                match msg.selector.as_str() {
                    // Auto-generate getters
                    $(
                        stringify!($field) => Some(self.$field.serialize()),
                    )*
                    
                    // Auto-generate setters
                    $(
                        concat!(stringify!($field), ":") => {
                            if let Some(value) = msg.args.first() {
                                if let Some(v) = <$type>::deserialize(value) {
                                    self.$field = v;
                                }
                            }
                            Some(Value::Nil)
                        }
                    )*
                    
                    // Init with dictionary
                    "init" => {
                        if let Some(Value::Dict(props)) = msg.args.first() {
                            $(
                                if let Some(value) = props.get(stringify!($field)) {
                                    if let Some(v) = <$type>::deserialize(value) {
                                        self.$field = v;
                                    }
                                }
                            )*
                        }
                        Some(Value::Nil)
                    }
                    
                    _ => None,
                }
            }
        }
        
        // Auto-generate serialization
        impl Object for $name {
            fn receive(&mut self, msg: &Message) -> Value {
                // First try auto-generated methods
                if let Some(result) = self.auto_receive(msg) {
                    return result;
                }
                
                // Then delegate to custom_receive if implemented
                self.custom_receive(msg)
            }
            
            fn serialize(&self) -> Value {
                dict! {
                    $(
                        stringify!($field) => self.$field.serialize()
                    ),*
                }
            }
            
            fn deserialize(&mut self, state: &Value) {
                if let Value::Dict(props) = state {
                    $(
                        if let Some(value) = props.get(stringify!($field)) {
                            if let Some(v) = <$type>::deserialize(value) {
                                self.$field = v;
                            }
                        }
                    )*
                }
            }
        }
        
        // Auto-generate registration function
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn register_objects(ctx: *mut std::ffi::c_void) {
            let register = unsafe { 
                &mut *(ctx as *mut Box<dyn FnMut(&str, Box<dyn Fn() -> Box<dyn Object>>)>) 
            };
            register(stringify!($name), Box::new(|| Box::new($name::default())));
        }
    };
}

// Helper macro to generate initWith selector  
#[macro_export]
macro_rules! init_with {
    ($self:ident, $msg:ident, $($field:ident),*) => {{
        let mut idx = 0;
        $(
            if let Some(value) = $msg.args.get(idx) {
                if let Some(v) = Deserialize::deserialize(value) {
                    $self.$field = v;
                }
            }
            idx += 1;
        )*
        Value::Nil
    }};
}



