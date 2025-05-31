// use std::any::{Any, TypeId};

// Re-export paste for use by downstream crates
pub use paste;

// TypedObject trait and related types - commented out since we're using direct calls now
/*
/// typed value that knows what it contains
#[derive(Debug)]
pub enum TypedValue {
    Unit,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Object(Box<dyn Any + Send + Sync>),
}

impl TypedValue {
    pub fn new<T: Any + Send + Sync + 'static>(val: T) -> Self {
        // special case common types for efficiency
        if TypeId::of::<T>() == TypeId::of::<()>() {
            TypedValue::Unit
        } else if TypeId::of::<T>() == TypeId::of::<bool>() {
            TypedValue::Bool(*(&val as &dyn Any).downcast_ref().unwrap())
        } else if TypeId::of::<T>() == TypeId::of::<i64>() {
            TypedValue::Int(*(&val as &dyn Any).downcast_ref().unwrap())
        } else if TypeId::of::<T>() == TypeId::of::<f64>() {
            TypedValue::Float(*(&val as &dyn Any).downcast_ref().unwrap())
        } else if TypeId::of::<T>() == TypeId::of::<String>() {
            TypedValue::String((&val as &dyn Any).downcast_ref::<String>().unwrap().clone())
        } else {
            TypedValue::Object(Box::new(val))
        }
    }

    pub fn get<T: Any + 'static>(&self) -> Option<&T> {
        match self {
            TypedValue::Unit if TypeId::of::<T>() == TypeId::of::<()>() => {
                Some((&() as &dyn Any).downcast_ref().unwrap())
            }
            TypedValue::Bool(b) if TypeId::of::<T>() == TypeId::of::<bool>() => {
                Some((b as &dyn Any).downcast_ref().unwrap())
            }
            TypedValue::Int(i) if TypeId::of::<T>() == TypeId::of::<i64>() => {
                Some((i as &dyn Any).downcast_ref().unwrap())
            }
            TypedValue::Float(f) if TypeId::of::<T>() == TypeId::of::<f64>() => {
                Some((f as &dyn Any).downcast_ref().unwrap())
            }
            TypedValue::String(s) if TypeId::of::<T>() == TypeId::of::<String>() => {
                Some((s as &dyn Any).downcast_ref().unwrap())
            }
            TypedValue::Object(obj) => obj.downcast_ref(),
            _ => None,
        }
    }

    pub fn type_id(&self) -> TypeId {
        match self {
            TypedValue::Unit => TypeId::of::<()>(),
            TypedValue::Bool(_) => TypeId::of::<bool>(),
            TypedValue::Int(_) => TypeId::of::<i64>(),
            TypedValue::Float(_) => TypeId::of::<f64>(),
            TypedValue::String(_) => TypeId::of::<String>(),
            TypedValue::Object(obj) => (**obj).type_id(),
        }
    }
}

/// message with typed arguments
pub struct TypedMessage {
    pub selector: String,
    pub args: Vec<TypedValue>,
}

/// describes a method's type signature
#[derive(Clone, Debug)]
pub struct MethodSignature {
    pub selector: String,
    pub arg_types: Vec<TypeId>,
    pub return_type: TypeId,
}

/// trait for objects that can receive typed messages
pub trait TypedObject: Any + Send + Sync {
    fn signatures(&self) -> &[MethodSignature];
    fn receive_typed(&mut self, msg: &TypedMessage) -> Result<TypedValue, String>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}
*/


/// helper macro to define typed objects with struct and methods
#[macro_export]
macro_rules! object {
    // version with auto accessors
    (
        $(#[$attr:meta])*
        pub struct $name:ident {
            $(pub $field:ident: $field_ty:ty),* $(,)?
        }
        
        accessors: [$($accessor_field:ident),* $(,)?]
        
        methods {
            $(
                fn $method:ident(&mut $self:ident $(, $arg:ident: $arg_ty:ty)*) $(-> $ret:ty)? $body:block
            )*
        }
    ) => {
        object! {
            $(#[$attr])*
            pub struct $name {
                $(pub $field: $field_ty),*
            }
            
            methods {
                // getters using field names
                $(
                    fn $accessor_field(&mut self) -> _ { self.$accessor_field }
                )*
                
                // user methods
                $(
                    fn $method(&mut $self $(, $arg: $arg_ty)*) $(-> $ret)? $body
                )*
            }
        }
    };
    
    // base version without accessors
    (
        $(#[$attr:meta])*
        pub struct $name:ident {
            $(pub $field:ident: $field_ty:ty),* $(,)?
        }
        
        methods {
            $(
                fn $method:ident(&mut $self:ident $(, $arg:ident: $arg_ty:ty)*) $(-> $ret:ty)? $body:block
            )*
        }
    ) => {
        $(#[$attr])*
        pub struct $name {
            $(pub $field: $field_ty),*
        }
        
        impl $name {
            $(
                fn $method(&mut $self $(, $arg: $arg_ty)*) $(-> $ret)? $body
            )*
        }
        
        // Generate no_mangle extern functions
        $crate::paste::paste! {
            // ABI version - const hash of structure
            #[unsafe(no_mangle)]
            #[allow(non_upper_case_globals)]
            pub static [<$name _abi_version>]: u64 = {
                // Simple const hash using string concatenation
                const fn const_hash(s: &str) -> u64 {
                    let mut hash = 0xcbf29ce484222325u64; // FNV offset basis
                    let bytes = s.as_bytes();
                    let mut i = 0;
                    while i < bytes.len() {
                        hash ^= bytes[i] as u64;
                        hash = hash.wrapping_mul(0x100000001b3u64); // FNV prime
                        i += 1;
                    }
                    hash
                }
                
                const_hash(concat!(
                    stringify!($name), ";",
                    $(stringify!($field), ":", stringify!($field_ty), ";",)*
                    $(stringify!($method), "(", $(stringify!($arg_ty), ",",)* ")", stringify!($(-> $ret)?), ";",)*
                ))
            };
            
            // Constructor if Default is implemented
            #[unsafe(no_mangle)]
            #[allow(non_snake_case)]
            pub extern "Rust" fn [<$name _default>]() -> Box<dyn ::std::any::Any> {
                Box::new(<$name as Default>::default())
            }
            
            // Getters
            $(
                #[unsafe(no_mangle)]
                #[allow(non_snake_case)]
                pub extern "Rust" fn [<$name _ $field>](obj: &dyn ::std::any::Any) -> $field_ty {
                    let Some(instance) = obj.downcast_ref::<$name>() else {
                        panic!(concat!("Type mismatch: expected ", stringify!($name)));
                    };
                    instance.$field.clone()
                }
            )*
            
            // Setters
            $(
                #[unsafe(no_mangle)]
                #[allow(non_snake_case)]
                pub extern "Rust" fn [<$name _set_ $field>](obj: &mut dyn ::std::any::Any, value: $field_ty) {
                    let Some(instance) = obj.downcast_mut::<$name>() else {
                        panic!(concat!("Type mismatch: expected ", stringify!($name)));
                    };
                    instance.$field = value;
                }
            )*
            
            // User methods
            $(
                #[unsafe(no_mangle)]
                #[allow(non_snake_case)]
                pub extern "Rust" fn [<$name _ $method>](obj: &mut dyn ::std::any::Any $(, $arg: $arg_ty)*) $(-> $ret)? {
                    let Some(instance) = obj.downcast_mut::<$name>() else {
                        panic!(concat!("Type mismatch: expected ", stringify!($name)));
                    };
                    instance.$method($($arg),*)
                }
            )*
        }
        
    };
    
    // helper to get return type, defaults to ()
    (@ret_type) => { () };
    (@ret_type $ret:ty) => { $ret };
}


#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectHandle(pub u64);
