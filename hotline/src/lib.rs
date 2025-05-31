use std::collections::HashMap;
use std::ffi::c_void;

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

// VTable entry - stores a function pointer that takes the object and a Value array
pub type VTableMethod = unsafe extern "C" fn(*mut c_void, &[Value]) -> Value;

// VTable structure that stores method pointers
#[repr(C)]
pub struct VTable {
    pub methods: HashMap<String, VTableMethod>,
    pub serialize: unsafe extern "C" fn(*const c_void) -> Value,
    pub deserialize: unsafe extern "C" fn(*mut c_void, *const Value),
}

// Trait for objects that can be dispatched via vtable
pub trait VTableObject: Send + Sync {
    fn get_vtable() -> &'static VTable
    where
        Self: Sized;
    fn as_ptr(&mut self) -> *mut c_void;
    fn as_const_ptr(&self) -> *const c_void;
}

// Legacy trait for compatibility
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

// Support for raw pointers
impl From<*mut u8> for Value {
    fn from(v: *mut u8) -> Self {
        Value::Int(v as i64)
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
    fn deserialize(value: &Value) -> Option<Self>
    where
        Self: Sized;
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

impl Deserialize for *mut u8 {
    fn deserialize(value: &Value) -> Option<Self> {
        match value {
            Value::Int(i) => Some(*i as *mut u8),
            _ => None,
        }
    }
}

// Additional implementations for Any trait
impl Serialize for *mut u8 {
    fn serialize(&self) -> Value {
        Value::Int(*self as i64)
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
        $crate::Value::Dict(map)
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

        $(
            method $method:ident $first_arg:ident : $first_type:ty $(, $part:ident : $arg:ident $arg_type:ty)* |$self_:ident| $body:block
        )*
    ) => {
        #[derive(Default)]
        pub struct $name {
            $(
                pub $field: $type,
            )*
        }

        impl $name {
            // Generate custom methods
            $(
                paste::paste! {
                    unsafe extern "C" fn [<vtable_ $method $(_ $part)*>](obj_ptr: *mut std::ffi::c_void, args: &[$crate::Value]) -> $crate::Value {
                        let $self_ = &mut *(obj_ptr as *mut $name);

                        let mut arg_idx = 0;
                        let $first_arg = match args.get(arg_idx) {
                            Some(value) => match <$first_type as $crate::Deserialize>::deserialize(value) {
                                Some(v) => v,
                                None => return $crate::Value::Nil,
                            },
                            None => return $crate::Value::Nil,
                        };
                        arg_idx += 1;

                        $(
                            let $arg = match args.get(arg_idx) {
                                Some(value) => match <$arg_type as $crate::Deserialize>::deserialize(value) {
                                    Some(v) => v,
                                    None => return $crate::Value::Nil,
                                },
                                None => return $crate::Value::Nil,
                            };
                            arg_idx += 1;
                        )*

                        $body
                    }
                }
            )*

            // Auto-generated getter methods
            $(
                paste::paste! {
                    unsafe extern "C" fn [<get_ $field>](obj_ptr: *mut std::ffi::c_void, _args: &[$crate::Value]) -> $crate::Value {
                        let obj = &*(obj_ptr as *const $name);
                        <$type as $crate::Serialize>::serialize(&obj.$field)
                    }
                }
            )*

            // Auto-generated setter methods
            $(
                paste::paste! {
                    unsafe extern "C" fn [<set_ $field>](obj_ptr: *mut std::ffi::c_void, args: &[$crate::Value]) -> $crate::Value {
                        let obj = &mut *(obj_ptr as *mut $name);

                        if let Some(value) = args.get(0) {
                            if let Some(v) = <$type as $crate::Deserialize>::deserialize(value) {
                                obj.$field = v;
                            }
                        }
                        $crate::Value::Nil
                    }
                }
            )*

            // Auto-generated initWith method
            unsafe extern "C" fn vtable_init_with(obj_ptr: *mut std::ffi::c_void, args: &[$crate::Value]) -> $crate::Value {
                let obj = &mut *(obj_ptr as *mut $name);
                let mut idx = 0;
                $(
                    if let Some(value) = args.get(idx) {
                        if let Some(v) = <$type as $crate::Deserialize>::deserialize(value) {
                            obj.$field = v;
                            eprintln!("Set {} to {:?}", stringify!($field), v);
                        } else {
                            eprintln!("Failed to deserialize {} from {:?}", stringify!($field), value);
                        }
                    } else {
                        eprintln!("No arg at index {} for {}", idx, stringify!($field));
                    }
                    idx += 1;
                )*
                $crate::Value::Nil
            }

            // Serialize method for vtable
            unsafe extern "C" fn vtable_serialize(obj_ptr: *const std::ffi::c_void) -> $crate::Value {
                let obj = &*(obj_ptr as *const $name);
                let mut map = std::collections::HashMap::new();
                $(
                    map.insert(stringify!($field).to_string(), <$type as $crate::Serialize>::serialize(&obj.$field));
                )*
                $crate::Value::Dict(map)
            }

            // Deserialize method for vtable
            unsafe extern "C" fn vtable_deserialize(obj_ptr: *mut std::ffi::c_void, state_ptr: *const $crate::Value) {
                let obj = &mut *(obj_ptr as *mut $name);
                let state = &*state_ptr;
                if let $crate::Value::Dict(props) = state {
                    $(
                        if let Some(value) = props.get(stringify!($field)) {
                            if let Some(v) = <$type as $crate::Deserialize>::deserialize(value) {
                                obj.$field = v;
                            }
                        }
                    )*
                }
            }
        }

        // Static vtable instance
        static mut VTABLE: Option<$crate::VTable> = None;
        static VTABLE_INIT: std::sync::Once = std::sync::Once::new();

        impl $crate::VTableObject for $name {
            fn get_vtable() -> &'static $crate::VTable {
                unsafe {
                    VTABLE_INIT.call_once(|| {
                        let mut methods = std::collections::HashMap::new();

                        // Register custom methods
                        $(
                            paste::paste! {
                                let selector = concat!(stringify!($method), $(":", stringify!($part),)* ":");
                                methods.insert(
                                    selector.to_string(),
                                    $name::[<vtable_ $method $(_ $part)*>] as $crate::VTableMethod
                                );
                            }
                        )*

                        // Register auto-generated getters
                        $(
                            paste::paste! {
                                methods.insert(
                                    stringify!($field).to_string(),
                                    $name::[<get_ $field>] as $crate::VTableMethod
                                );
                            }
                        )*

                        // Register auto-generated setters
                        $(
                            paste::paste! {
                                methods.insert(
                                    concat!(stringify!($field), ":").to_string(),
                                    $name::[<set_ $field>] as $crate::VTableMethod
                                );
                            }
                        )*

                        // Register initWith method
                        let field_names = vec![$(stringify!($field),)*];
                        if field_names.len() > 0 {
                            let mut init_selector = String::from("initWith");
                            for (i, field) in field_names.iter().enumerate() {
                                if i == 0 {
                                    // Capitalize first letter of first field
                                    let mut chars = field.chars();
                                    if let Some(first) = chars.next() {
                                        init_selector.push_str(&first.to_uppercase().to_string());
                                        init_selector.push_str(&chars.as_str());
                                    }
                                    init_selector.push(':');
                                } else {
                                    init_selector.push_str(field);
                                    init_selector.push(':');
                                }
                            }
                            eprintln!("Generated initWith selector: {}", init_selector);
                            methods.insert(init_selector, $name::vtable_init_with as $crate::VTableMethod);
                        }

                        VTABLE = Some($crate::VTable {
                            methods,
                            serialize: $name::vtable_serialize,
                            deserialize: $name::vtable_deserialize,
                        });
                    });
                    VTABLE.as_ref().unwrap()
                }
            }

            fn as_ptr(&mut self) -> *mut std::ffi::c_void {
                self as *mut _ as *mut std::ffi::c_void
            }

            fn as_const_ptr(&self) -> *const std::ffi::c_void {
                self as *const _ as *const std::ffi::c_void
            }
        }

        // Legacy Object trait implementation for compatibility
        impl $crate::Object for $name {
            fn receive(&mut self, msg: &$crate::Message) -> $crate::Value {
                // Dispatch through vtable
                let vtable = <$name as $crate::VTableObject>::get_vtable();
                eprintln!("{} receive: selector={}", stringify!($name), msg.selector);
                if let Some(method) = vtable.methods.get(&msg.selector) {
                    eprintln!("Found method for selector {}", msg.selector);
                    unsafe {
                        method(<$name as $crate::VTableObject>::as_ptr(self), &msg.args)
                    }
                } else {
                    eprintln!("No method found for selector {}", msg.selector);
                    eprintln!("Available methods: {:?}", vtable.methods.keys().collect::<Vec<_>>());
                    $crate::Value::Nil
                }
            }

            fn serialize(&self) -> $crate::Value {
                unsafe {
                    let vtable = <$name as $crate::VTableObject>::get_vtable();
                    (vtable.serialize)(<$name as $crate::VTableObject>::as_const_ptr(self))
                }
            }

            fn deserialize(&mut self, state: &$crate::Value) {
                unsafe {
                    let vtable = <$name as $crate::VTableObject>::get_vtable();
                    (vtable.deserialize)(<$name as $crate::VTableObject>::as_ptr(self), state as *const $crate::Value)
                }
            }
        }

        // Auto-generate registration function
        #[unsafe(no_mangle)]
        pub unsafe extern "C" fn register_objects(ctx: *mut std::ffi::c_void) {
            let register = unsafe {
                &mut *(ctx as *mut Box<dyn FnMut(&str, Box<dyn Fn() -> Box<dyn $crate::Object>>)>)
            };
            register(stringify!($name), Box::new(|| Box::new($name::default())));
        }
    };
}
