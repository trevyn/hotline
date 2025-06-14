use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::marker::PhantomData;

use crate::{ObjectHandle, get_object_by_id};

// Wrapper type for serializing object references as IDs
#[derive(Serialize, Deserialize)]
pub struct ObjectRef {
    pub id: u64,
    pub type_name: String,
}

// Custom serialization for object handles
pub fn serialize_object_handle<S>(handle: &ObjectHandle, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    if let Ok(guard) = handle.lock() {
        let obj_ref = ObjectRef { id: guard.object_id(), type_name: guard.type_name().to_string() };
        obj_ref.serialize(serializer)
    } else {
        serializer.serialize_none()
    }
}

pub fn deserialize_object_handle<'de, D>(deserializer: D) -> Result<ObjectHandle, D::Error>
where
    D: Deserializer<'de>,
{
    let obj_ref = ObjectRef::deserialize(deserializer)?;

    // Look up the object by ID
    get_object_by_id(obj_ref.id).ok_or_else(|| de::Error::custom(format!("Object with ID {} not found", obj_ref.id)))
}

// Serialization helpers for Option<T> where T is an object type
pub mod option_object {
    use super::*;

    pub fn serialize<S, T>(opt: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: AsRef<ObjectHandle>,
    {
        match opt {
            Some(obj) => serialize_object_handle(obj.as_ref(), serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
    where
        D: Deserializer<'de>,
        T: From<ObjectHandle>,
    {
        struct OptionObjectVisitor<T> {
            marker: PhantomData<T>,
        }

        impl<'de, T> Visitor<'de> for OptionObjectVisitor<T>
        where
            T: From<ObjectHandle>,
        {
            type Value = Option<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an optional object reference")
            }

            fn visit_none<E>(self) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(None)
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                let handle = deserialize_object_handle(deserializer)?;
                Ok(Some(T::from(handle)))
            }
        }

        deserializer.deserialize_option(OptionObjectVisitor { marker: PhantomData })
    }
}

// Macro to generate object wrapper types that implement AsRef<ObjectHandle> and From<ObjectHandle>
#[macro_export]
macro_rules! object_wrapper {
    ($name:ident) => {
        #[derive(Clone)]
        pub struct $name(pub ::hotline::ObjectHandle);

        impl AsRef<::hotline::ObjectHandle> for $name {
            fn as_ref(&self) -> &::hotline::ObjectHandle {
                &self.0
            }
        }

        impl From<::hotline::ObjectHandle> for $name {
            fn from(handle: ::hotline::ObjectHandle) -> Self {
                $name(handle)
            }
        }

        impl ::hotline::serde::Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: ::hotline::serde::Serializer,
            {
                ::hotline::object_serde::serialize_object_handle(&self.0, serializer)
            }
        }

        impl<'de> ::hotline::serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: ::hotline::serde::Deserializer<'de>,
            {
                ::hotline::object_serde::deserialize_object_handle(deserializer).map($name)
            }
        }
    };
}
