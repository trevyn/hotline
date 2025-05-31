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
        
        // Generate extern functions with signature-encoded names
        $crate::paste::paste! {
            // Constructor if Default is implemented
            #[unsafe(no_mangle)]
            #[allow(non_snake_case)]
            pub extern "Rust" fn [<$name __new____to__Box_dyn_Any>]() -> Box<dyn ::std::any::Any> {
                Box::new(<$name as Default>::default())
            }
            
            // Getters - encode full signature in name
            $(
                #[unsafe(no_mangle)]
                #[allow(non_snake_case)]
                pub extern "Rust" fn [<$name __get_ $field ____obj_ref_dyn_Any__to__ $field_ty>](obj: &dyn ::std::any::Any) -> $field_ty {
                    let Some(instance) = obj.downcast_ref::<$name>() else {
                        panic!(concat!("Type mismatch: expected ", stringify!($name)));
                    };
                    instance.$field.clone()
                }
            )*
            
            // Setters - encode full signature in name
            $(
                #[unsafe(no_mangle)]
                #[allow(non_snake_case)]
                pub extern "Rust" fn [<$name __set_ $field ____obj_mut_dyn_Any__ $field _ $field_ty __to__unit>](obj: &mut dyn ::std::any::Any, value: $field_ty) {
                    let Some(instance) = obj.downcast_mut::<$name>() else {
                        panic!(concat!("Type mismatch: expected ", stringify!($name)));
                    };
                    instance.$field = value;
                }
            )*
            
            // User methods - encode full signature in name
            $(
                #[unsafe(no_mangle)]
                #[allow(non_snake_case)]
                pub extern "Rust" fn [<$name __ $method ____obj_mut_dyn_Any $(__$arg _ $arg_ty)* __to__ $($ret)? unit>](obj: &mut dyn ::std::any::Any $(, $arg: $arg_ty)*) $(-> $ret)? {
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
    
    // helper to get return type name for symbol
    (@ret_type_name) => { unit };
    (@ret_type_name $ret:ty) => { $ret };
    
    // helper to generate method with return type
    (@gen_method $name:ident, $method:ident, $self_name:ident, $(($arg:ident, $arg_ty:ty),)* $ret:ty) => {
        $crate::paste::paste! {
            #[unsafe(no_mangle)]
            #[allow(non_snake_case)]
            pub extern "Rust" fn [<hotline__ $name __ $method ____obj_mut_dyn_Any $(__$arg _ $arg_ty)* __to__ $ret>]($self_name: &mut dyn ::std::any::Any $(, $arg: $arg_ty)*) -> $ret {
                let Some(instance) = $self_name.downcast_mut::<$name>() else {
                    panic!(concat!("Type mismatch: expected ", stringify!($name)));
                };
                instance.$method($($arg),*)
            }
        }
    };
    
    // helper to generate method without return type (returns unit)
    (@gen_method $name:ident, $method:ident, $self_name:ident, $(($arg:ident, $arg_ty:ty),)*) => {
        $crate::paste::paste! {
            #[unsafe(no_mangle)]
            #[allow(non_snake_case)]
            pub extern "Rust" fn [<hotline__ $name __ $method ____obj_mut_dyn_Any $(__$arg _ $arg_ty)* __to__unit>]($self_name: &mut dyn ::std::any::Any $(, $arg: $arg_ty)*) {
                let Some(instance) = $self_name.downcast_mut::<$name>() else {
                    panic!(concat!("Type mismatch: expected ", stringify!($name)));
                };
                instance.$method($($arg),*)
            }
        }
    };
    
    // helper to generate method with full signature encoding - no return type
    (@gen_method_with_sig $name:ident, $method:ident, $self_name:ident, $(($arg:ident, $arg_ty:ty),)* ; ) => {
        $crate::paste::paste! {
            #[export_name = concat!(stringify!($name), "__", stringify!($method), "____obj_mut_dyn_Any", $(concat!("__", stringify!($arg), "_", stringify!($arg_ty))),*, "__to__unit")]
            #[allow(non_snake_case)]
            pub extern "Rust" fn [<$name __ $method>]($self_name: &mut dyn ::std::any::Any $(, $arg: $arg_ty)*) {
                let Some(instance) = $self_name.downcast_mut::<$name>() else {
                    panic!(concat!("Type mismatch: expected ", stringify!($name)));
                };
                instance.$method($($arg),*)
            }
        }
    };
    
    // helper to generate method with full signature encoding - with return type
    (@gen_method_with_sig $name:ident, $method:ident, $self_name:ident, $(($arg:ident, $arg_ty:ty),)* ; $ret:ty) => {
        $crate::paste::paste! {
            #[export_name = concat!(stringify!($name), "__", stringify!($method), "____obj_mut_dyn_Any", $(concat!("__", stringify!($arg), "_", stringify!($arg_ty))),*, "__to__", stringify!($ret))]
            #[allow(non_snake_case)]
            pub extern "Rust" fn [<$name __ $method>]($self_name: &mut dyn ::std::any::Any $(, $arg: $arg_ty)*) -> $ret {
                let Some(instance) = $self_name.downcast_mut::<$name>() else {
                    panic!(concat!("Type mismatch: expected ", stringify!($name)));
                };
                instance.$method($($arg),*)
            }
        }
    };
    
    // helper to get return type as string
    (@ret_str) => { "()" };
    (@ret_str $ret:ty) => { stringify!($ret) };
}


#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectHandle(pub u64);
