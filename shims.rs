// Auto-generated shims
use crate::{DirectRuntime, ObjectHandle};
use std::sync::{Arc, Mutex};

/// Auto-generated shim for Rect
pub struct Rect {
    runtime: std::sync::Arc<std::sync::Mutex<DirectRuntime>>,
    handle: ObjectHandle,
}

impl Rect {
    pub fn new(runtime: std::sync::Arc<std::sync::Mutex<DirectRuntime>>, handle: ObjectHandle) -> Self {
        Self { runtime, handle }
    }

    pub fn get_height(&self) -> Result<f64, Box<dyn std::error::Error>> {
        self.runtime.lock().unwrap().call_getter::<f64>(
            self.handle,
            "Rect",
            "libRect",
            "get_height"
        )
    }

    pub fn get_width(&self) -> Result<f64, Box<dyn std::error::Error>> {
        self.runtime.lock().unwrap().call_getter::<f64>(
            self.handle,
            "Rect",
            "libRect",
            "get_width"
        )
    }

    pub fn get_x(&self) -> Result<f64, Box<dyn std::error::Error>> {
        self.runtime.lock().unwrap().call_getter::<f64>(
            self.handle,
            "Rect",
            "libRect",
            "get_x"
        )
    }

    pub fn get_y(&self) -> Result<f64, Box<dyn std::error::Error>> {
        self.runtime.lock().unwrap().call_getter::<f64>(
            self.handle,
            "Rect",
            "libRect",
            "get_y"
        )
    }

    pub fn move_by(&self, dx: f64, dy: f64) -> Result<(), Box<dyn std::error::Error>> {
        let args: Vec<Box<dyn std::any::Any>> = vec![Box::new(dx), Box::new(dy)];
        self.runtime.lock().unwrap().call_method(
            self.handle,
            "Rect",
            "libRect",
            "move_by",
            args
        )?;
        Ok(())
    }

    pub fn render(&self, buffer: &mut [u8], buffer_width: i64, buffer_height: i64, pitch: i64) -> Result<(), Box<dyn std::error::Error>> {
        let args: Vec<Box<dyn std::any::Any>> = vec![Box::new(buffer), Box::new(buffer_width), Box::new(buffer_height), Box::new(pitch)];
        self.runtime.lock().unwrap().call_method(
            self.handle,
            "Rect",
            "libRect",
            "render",
            args
        )?;
        Ok(())
    }

    pub fn set_height(&self, value: f64) -> Result<(), Box<dyn std::error::Error>> {
        self.runtime.lock().unwrap().call_setter(
            self.handle,
            "Rect",
            "libRect",
            "set_height",
            value
        )
    }

    pub fn set_width(&self, value: f64) -> Result<(), Box<dyn std::error::Error>> {
        self.runtime.lock().unwrap().call_setter(
            self.handle,
            "Rect",
            "libRect",
            "set_width",
            value
        )
    }

    pub fn set_x(&self, value: f64) -> Result<(), Box<dyn std::error::Error>> {
        self.runtime.lock().unwrap().call_setter(
            self.handle,
            "Rect",
            "libRect",
            "set_x",
            value
        )
    }

    pub fn set_y(&self, value: f64) -> Result<(), Box<dyn std::error::Error>> {
        self.runtime.lock().unwrap().call_setter(
            self.handle,
            "Rect",
            "libRect",
            "set_y",
            value
        )
    }

}


