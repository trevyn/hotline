use std::any::{Any, TypeId};

// Re-export paste for use by downstream crates
pub use paste;

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
        
        $crate::paste::paste! {
            $crate::typed_methods! {
                $name {
                    // auto-generated getters (field name as method)
                    $(
                        fn $field(&mut self) -> $field_ty { self.$field }
                    )*
                    
                    // auto-generated setters with set_ prefix
                    $(
                        fn [<set_ $field>](&mut self, value: $field_ty) { self.$field = value; }
                    )*
                    
                    // user-defined methods
                    $(
                        fn $method(&mut $self $(, $arg: $arg_ty)*) $(-> $ret)? $body
                    )*
                }
            }
        }
    };
}

/// helper macro to define typed methods
#[macro_export]
macro_rules! typed_methods {
    (
        $obj:ty {
            $(
                fn $method:ident(&mut $self:ident $(, $arg:ident: $arg_ty:ty)*) $(-> $ret:ty)? $body:block
            )*
        }
    ) => {
        impl $obj {
            $(
                fn $method(&mut $self $(, $arg: $arg_ty)*) $(-> $ret)? $body
            )*
        }

        impl TypedObject for $obj {
            fn signatures(&self) -> &[MethodSignature] {
                use std::any::TypeId;
                use std::sync::OnceLock;
                static SIGS: OnceLock<Vec<MethodSignature>> = OnceLock::new();
                SIGS.get_or_init(|| vec![
                    $(
                        MethodSignature {
                            selector: stringify!($method).to_string(),
                            arg_types: vec![$(TypeId::of::<$arg_ty>()),*],
                            return_type: TypeId::of::<typed_methods!(@ret_type $($ret)?)>(),
                        },
                    )*
                ])
            }

            fn receive_typed(&mut self, msg: &TypedMessage) -> Result<TypedValue, String> {
                match msg.selector.as_str() {
                    $(
                        stringify!($method) => {
                            // extract args with type checking
                            let mut _arg_idx = 0;
                            $(
                                let $arg = msg.args.get(_arg_idx)
                                    .ok_or(format!("missing arg {}", _arg_idx))?
                                    .get::<$arg_ty>()
                                    .ok_or(format!("arg {} type mismatch", _arg_idx))?
    ;
                                _arg_idx += 1;
                            )*

                            let result = self.$method($($arg.clone()),*);
                            Ok(TypedValue::new(result))
                        }
                    )*
                    _ => Err(format!("unknown selector: {}", msg.selector)),
                }
            }

            fn as_any(&self) -> &dyn std::any::Any { self }
            fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
        }
    };

    // helper to get return type, defaults to ()
    (@ret_type) => { () };
    (@ret_type $ret:ty) => { $ret };
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectHandle(pub u64);

#[cfg(feature = "monolith")]
pub mod monolith;

// helper to convert type to consistent handle
pub trait TypeToHandle {
    fn type_handle() -> ObjectHandle;
}

impl<T: 'static> TypeToHandle for T {
    fn type_handle() -> ObjectHandle {
        // use type hash as handle for monolith builds
        use std::hash::{Hash, Hasher};
        use std::collections::hash_map::DefaultHasher;
        
        let mut hasher = DefaultHasher::new();
        std::any::TypeId::of::<T>().hash(&mut hasher);
        ObjectHandle(hasher.finish())
    }
}
