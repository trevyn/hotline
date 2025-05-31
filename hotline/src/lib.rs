pub use paste;

/// helper macro to define typed objects with struct and methods
#[macro_export]
macro_rules! object {
    // new syntax with impl block
    ({
        $(#[$attr:meta])*
        pub struct $name:ident {
            $(pub $field:ident: $field_ty:ty),* $(,)?
        }

        impl $impl_name:ident {
            $(
                fn $method:ident(&mut $self:ident $(, $arg:ident: $arg_ty:ty)*) $(-> $ret:ty)? $body:block
            )*
        }
    }) => {
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
            pub extern "Rust" fn [<$name __new____to__Box_lt_dyn_Any_gt>]() -> Box<dyn ::std::any::Any> {
                Box::new(<$name as Default>::default())
            }

        }

        // Generate getters and setters separately to work around paste limitations
        $(
            object!(@gen_getter $name, $field, $field_ty);
        )*

        $(
            object!(@gen_setter $name, $field, $field_ty);
        )*

        // User methods - encode full signature in name
        $(
            object!(@gen_user_method $name, $method, $(($arg, $arg_ty),)* $($ret)?);
        )*

    };


    // Generate getter
    (@gen_getter $name:ident, $field:ident, f64) => {
        $crate::paste::paste! {
            #[unsafe(no_mangle)]
            #[allow(non_snake_case)]
            pub extern "Rust" fn [<$name __get_ $field ____obj_ref_dyn_Any__to__f64>](obj: &dyn ::std::any::Any) -> f64 {
                let Some(instance) = obj.downcast_ref::<$name>() else {
                    panic!(concat!("Type mismatch: expected ", stringify!($name)));
                };
                instance.$field.clone()
            }
        }
    };

    (@gen_getter $name:ident, $field:ident, $field_ty:ty) => {
        // For now, fallback to simple type names for non-primitive types
        $crate::paste::paste! {
            #[unsafe(no_mangle)]
            #[allow(non_snake_case)]
            pub extern "Rust" fn [<$name __get_ $field ____obj_ref_dyn_Any__to__ $field_ty>](obj: &dyn ::std::any::Any) -> $field_ty {
                let Some(instance) = obj.downcast_ref::<$name>() else {
                    panic!(concat!("Type mismatch: expected ", stringify!($name)));
                };
                instance.$field.clone()
            }
        }
    };

    // Generate setter
    (@gen_setter $name:ident, $field:ident, f64) => {
        $crate::paste::paste! {
            #[unsafe(no_mangle)]
            #[allow(non_snake_case)]
            pub extern "Rust" fn [<$name __set_ $field ____obj_mut_dyn_Any__ $field _f64__to__unit>](obj: &mut dyn ::std::any::Any, value: f64) {
                let Some(instance) = obj.downcast_mut::<$name>() else {
                    panic!(concat!("Type mismatch: expected ", stringify!($name)));
                };
                instance.$field = value;
            }
        }
    };

    (@gen_setter $name:ident, $field:ident, $field_ty:ty) => {
        // For now, fallback to simple type names for non-primitive types
        $crate::paste::paste! {
            #[unsafe(no_mangle)]
            #[allow(non_snake_case)]
            pub extern "Rust" fn [<$name __set_ $field ____obj_mut_dyn_Any__ $field _ $field_ty __to__unit>](obj: &mut dyn ::std::any::Any, value: $field_ty) {
                let Some(instance) = obj.downcast_mut::<$name>() else {
                    panic!(concat!("Type mismatch: expected ", stringify!($name)));
                };
                instance.$field = value;
            }
        }
    };

    // generate user method with encoded type names
    (@gen_user_method $name:ident, $method:ident, $(($arg:ident, $arg_ty:ty),)*) => {
        $crate::paste::paste! {
            #[unsafe(no_mangle)]
            #[allow(non_snake_case)]
            pub extern "Rust" fn [<$name __ $method ____obj_mut_dyn_Any $(__ $arg _ stringify!($arg_ty):type)* __to__unit>](
                obj: &mut dyn ::std::any::Any $(, $arg: $arg_ty)*
            ) {
                let Some(instance) = obj.downcast_mut::<$name>() else {
                    panic!(concat!("Type mismatch: expected ", stringify!($name)));
                };
                instance.$method($($arg),*)
            }
        }
    };

    (@gen_user_method $name:ident, $method:ident, $(($arg:ident, $arg_ty:ty),)* $ret:ty) => {
        $crate::paste::paste! {
            #[unsafe(no_mangle)]
            #[allow(non_snake_case)]
            pub extern "Rust" fn [<$name __ $method ____obj_mut_dyn_Any $(__ $arg _ stringify!($arg_ty):type)* __to__ stringify!($ret):type>](
                obj: &mut dyn ::std::any::Any $(, $arg: $arg_ty)*
            ) -> $ret {
                let Some(instance) = obj.downcast_mut::<$name>() else {
                    panic!(concat!("Type mismatch: expected ", stringify!($name)));
                };
                instance.$method($($arg),*)
            }
        }
    };
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjectHandle(pub u64);
