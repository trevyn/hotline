use crate::*;

// Include auto-generated object implementations
include!(env!("MONOLITH_OBJECTS_PATH"));

// In monolith mode, handles ARE the objects - zero indirection
#[derive(Debug)]
pub enum AllObjects {
    Rect(rect::Rect),
    // Add more object types here as you add them to your project
}

impl AllObjects {
    // Zero-cost dispatch - compiler can inline everything
    pub fn receive_typed(&mut self, msg: &TypedMessage) -> Result<TypedValue, String> {
        match self {
            AllObjects::Rect(r) => r.receive_typed(msg),
        }
    }
    
    pub fn signatures(&self) -> &[MethodSignature] {
        match self {
            AllObjects::Rect(r) => r.signatures(),
        }
    }
    
    pub fn as_any(&self) -> &dyn Any {
        match self {
            AllObjects::Rect(r) => r.as_any(),
        }
    }
    
    pub fn as_any_mut(&mut self) -> &mut dyn Any {
        match self {
            AllObjects::Rect(r) => r.as_any_mut(),
        }
    }
}

// Static runtime with zero dynamic dispatch
pub struct StaticRuntime {
    objects: Vec<Option<AllObjects>>,
}

impl StaticRuntime {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
        }
    }
    
    pub fn register(&mut self, obj: AllObjects) -> ObjectHandle {
        let handle = ObjectHandle(self.objects.len() as u64);
        self.objects.push(Some(obj));
        handle
    }
    
    pub fn send(&mut self, target: ObjectHandle, msg: TypedMessage) -> Result<TypedValue, String> {
        let obj = self.objects.get_mut(target.0 as usize)
            .and_then(|opt| opt.as_mut())
            .ok_or_else(|| format!("no object with handle {:?}", target))?;
        
        // Direct dispatch - no vtable lookup!
        obj.receive_typed(&msg)
    }
    
    pub fn get_object(&self, handle: ObjectHandle) -> Option<&AllObjects> {
        self.objects.get(handle.0 as usize)
            .and_then(|opt| opt.as_ref())
    }
    
    pub fn get_object_mut(&mut self, handle: ObjectHandle) -> Option<&mut AllObjects> {
        self.objects.get_mut(handle.0 as usize)
            .and_then(|opt| opt.as_mut())
    }
    
    // Type-safe accessors that compile to direct memory access
    pub fn get_rect(&self, handle: ObjectHandle) -> Option<&rect::Rect> {
        match self.get_object(handle)? {
            AllObjects::Rect(r) => Some(r),
        }
    }
    
    pub fn get_rect_mut(&mut self, handle: ObjectHandle) -> Option<&mut rect::Rect> {
        match self.get_object_mut(handle)? {
            AllObjects::Rect(r) => Some(r),
        }
    }
}

// Macro for direct method calls - compiles to zero-cost dispatch
#[macro_export]
macro_rules! static_call {
    ($runtime:expr, $handle:expr, $method:ident($($arg:expr),*)) => {{
        match $runtime.get_object_mut($handle) {
            Some($crate::monolith::AllObjects::Rect(r)) => {
                Ok($crate::TypedValue::new(r.$method($($arg),*)))
            }
            None => Err("object not found".to_string()),
        }
    }};
}

// Even more direct - if you know the type at compile time
#[macro_export]
macro_rules! rect_call {
    ($runtime:expr, $handle:expr, $method:ident($($arg:expr),*)) => {{
        $runtime.get_rect_mut($handle)
            .map(|r| $crate::TypedValue::new(r.$method($($arg),*)))
            .ok_or_else(|| "rect not found".to_string())
    }};
}