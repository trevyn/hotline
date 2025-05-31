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
    
    // Dynamic cast support
    fn as_any(&self) -> &dyn std::any::Any;
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

// Bounds protocol for renderables
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Bounds {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self { x, y, width, height }
    }
    
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width &&
        y >= self.y && y <= self.y + self.height
    }
    
    pub fn intersects(&self, other: &Bounds) -> bool {
        self.x < other.x + other.width &&
        self.x + self.width > other.x &&
        self.y < other.y + other.height &&
        self.y + self.height > other.y
    }
}

impl Serialize for Bounds {
    fn serialize(&self) -> Value {
        dict! {
            "x" => Value::Float(self.x),
            "y" => Value::Float(self.y),
            "width" => Value::Float(self.width),
            "height" => Value::Float(self.height)
        }
    }
}

impl Deserialize for Bounds {
    fn deserialize(value: &Value) -> Option<Self> {
        if let Value::Dict(props) = value {
            Some(Bounds {
                x: props.get("x").and_then(f64::deserialize)?,
                y: props.get("y").and_then(f64::deserialize)?,
                width: props.get("width").and_then(f64::deserialize)?,
                height: props.get("height").and_then(f64::deserialize)?,
            })
        } else {
            None
        }
    }
}

// Trait for objects that can be rendered
pub trait Renderable: Object {
    fn bounds(&self) -> Bounds;
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
    // Support for methods with arguments only
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
        object!(@impl
            $name {
                $(
                    $field: $type
                ),*
            }
            
            [] // no-arg methods
            
            [$(
                ($method $first_arg : $first_type $(, $part : $arg $arg_type)* |$self_| $body)
            )*]
        );
    };
    
    // Support for methods without arguments only
    (
        $name:ident {
            $(
                $field:ident: $type:ty
            ),* $(,)?
        }

        $(
            method $method:ident |$self_:ident| $body:block
        )*
    ) => {
        object!(@impl
            $name {
                $(
                    $field: $type
                ),*
            }
            
            [$(
                ($method |$self_| $body)
            )*]
            
            [] // arg methods
        );
    };
    
    // Support for mix of both method types - using token tree matching
    (
        $name:ident {
            $(
                $field:ident: $type:ty
            ),* $(,)?
        }
        
        $($method_tokens:tt)*
    ) => {
        object!(@parse
            $name {
                $(
                    $field: $type
                ),*
            }
            [] // no-arg methods
            [] // arg methods
            $($method_tokens)*
        );
    };
    
    // Parse method without arguments
    (@parse
        $name:ident { $($fields:tt)* }
        [$($noarg_methods:tt)*]
        [$($arg_methods:tt)*]
        method $method:ident |$self_:ident| $body:block
        $($rest:tt)*
    ) => {
        object!(@parse
            $name { $($fields)* }
            [$($noarg_methods)* ($method |$self_| $body)]
            [$($arg_methods)*]
            $($rest)*
        );
    };
    
    // Parse method with arguments
    (@parse
        $name:ident { $($fields:tt)* }
        [$($noarg_methods:tt)*]
        [$($arg_methods:tt)*]
        method $method:ident $first_arg:ident : $first_type:ty $(, $part:ident : $arg:ident $arg_type:ty)* |$self_:ident| $body:block
        $($rest:tt)*
    ) => {
        object!(@parse
            $name { $($fields)* }
            [$($noarg_methods)*]
            [$($arg_methods)* ($method $first_arg : $first_type $(, $part : $arg $arg_type)* |$self_| $body)]
            $($rest)*
        );
    };
    
    // Done parsing - call impl
    (@parse
        $name:ident { $($fields:tt)* }
        [$($noarg_methods:tt)*]
        [$($arg_methods:tt)*]
    ) => {
        object!(@impl
            $name { $($fields)* }
            [$($noarg_methods)*]
            [$($arg_methods)*]
        );
    };
    
    // Implementation
    (@impl
        $name:ident {
            $(
                $field:ident: $type:ty
            ),* $(,)?
        }
        
        [$(
            ($method_noargs:ident |$self_noargs:ident| $body_noargs:block)
        )*]
        
        [$(
            ($method:ident $first_arg:ident : $first_type:ty $(, $part:ident : $arg:ident $arg_type:ty)* |$self_:ident| $body:block)
        )*]
    ) => {
        #[derive(Default)]
        pub struct $name {
            $(
                pub $field: $type,
            )*
        }

        impl $name {
            // Auto-generated initWith method
            unsafe extern "C" fn vtable_init_with(obj_ptr: *mut std::ffi::c_void, args: &[$crate::Value]) -> $crate::Value {
                unsafe {
                    let obj = &mut *(obj_ptr as *mut $name);
                    let mut idx = 0;
                    $(
                        if let Some(value) = args.get(idx) {
                            if let Some(v) = <$type as $crate::Deserialize>::deserialize(value) {
                                obj.$field = v;
                            }
                        }
                        idx += 1;
                    )*
                }
                $crate::Value::Nil
            }

            // Serialize method for vtable
            unsafe extern "C" fn vtable_serialize(obj_ptr: *const std::ffi::c_void) -> $crate::Value {
                unsafe {
                    let obj = &*(obj_ptr as *const $name);
                    let mut map = std::collections::HashMap::new();
                    $(
                        map.insert(stringify!($field).to_string(), <$type as $crate::Serialize>::serialize(&obj.$field));
                    )*
                    $crate::Value::Dict(map)
                }
            }

            // Deserialize method for vtable
            unsafe extern "C" fn vtable_deserialize(obj_ptr: *mut std::ffi::c_void, state_ptr: *const $crate::Value) {
                unsafe {
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
        }

        // Static vtable instance
        static mut VTABLE: Option<$crate::VTable> = None;
        static VTABLE_INIT: std::sync::Once = std::sync::Once::new();

        impl $crate::VTableObject for $name {
            fn get_vtable() -> &'static $crate::VTable {
                unsafe {
                    VTABLE_INIT.call_once(|| {
                        let mut methods = std::collections::HashMap::new();

                        // Since we can't generate unique function names without paste,
                        // we'll use the legacy Object trait approach for methods
                        // This maintains compatibility while removing the paste dependency

                        // Note: Since we can't generate unique function names without paste,
                        // we don't register getters/setters in the vtable.
                        // They're handled through the Object trait's receive method instead.

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
                // Handle custom methods directly in receive
                match msg.selector.as_str() {
                    // Methods without arguments
                    $(
                        stringify!($method_noargs) => {
                            let $self_noargs = self;
                            $body_noargs
                        }
                    )*
                    // Methods with arguments
                    $(
                        concat!(stringify!($method), $(":", stringify!($part),)* ":") => {
                            let $self_ = self;

                            let mut arg_idx = 0;
                            let $first_arg = match msg.args.get(arg_idx) {
                                Some(value) => match <$first_type as $crate::Deserialize>::deserialize(value) {
                                    Some(v) => v,
                                    None => return $crate::Value::Nil,
                                },
                                None => return $crate::Value::Nil,
                            };
                            arg_idx += 1;

                            $(
                                let $arg = match msg.args.get(arg_idx) {
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
                    )*
                    _ => {
                        // Handle auto-generated getters
                        $(
                            if msg.selector == stringify!($field) {
                                return <$type as $crate::Serialize>::serialize(&self.$field);
                            }
                        )*
                        
                        // Handle auto-generated setters
                        $(
                            if msg.selector == concat!(stringify!($field), ":") {
                                if let Some(value) = msg.args.get(0) {
                                    if let Some(v) = <$type as $crate::Deserialize>::deserialize(value) {
                                        self.$field = v;
                                    }
                                }
                                return $crate::Value::Nil;
                            }
                        )*
                        
                        // Dispatch through vtable for other auto-generated methods
                        let vtable = <$name as $crate::VTableObject>::get_vtable();
                        if let Some(method) = vtable.methods.get(&msg.selector) {
                            unsafe {
                                method(<$name as $crate::VTableObject>::as_ptr(self), &msg.args)
                            }
                        } else {
                            $crate::Value::Nil
                        }
                    }
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
            
            fn as_any(&self) -> &dyn std::any::Any {
                self
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