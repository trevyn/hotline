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
}

// Conversion helpers
impl From<i32> for Value {
    fn from(v: i32) -> Self { Value::Int(v as i64) }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self { Value::Int(v) }
}

impl From<f32> for Value {
    fn from(v: f32) -> Self { Value::Float(v as f64) }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self { Value::Float(v) }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self { Value::String(v.to_string()) }
}

impl From<String> for Value {
    fn from(v: String) -> Self { Value::String(v) }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self { Value::Bool(v) }
}

impl From<ObjectHandle> for Value {
    fn from(v: ObjectHandle) -> Self { Value::Object(v) }
}

pub fn to_value<T: Into<Value>>(v: T) -> Value {
    v.into()
}

// Function type for object registration - using opaque pointer for FFI safety
pub type RegisterFn = unsafe extern "C" fn(*mut std::ffi::c_void);