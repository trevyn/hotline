// Auto-generated proxy module for HighlightLens
// Include this module in objects that need to call HighlightLens methods

use hotline::ObjectRef;

/// Type marker for HighlightLens
pub struct HighlightLens;

/// Extension methods for ObjectRef<HighlightLens>
pub trait HighlightLensProxy {
    fn set_target(&mut self, target : ObjectHandle);
    fn set_highlight_color(&mut self, b : u8, g : u8, r : u8, a : u8);
    fn render(&mut self, buffer : & mut [u8], buffer_width : i64, buffer_height : i64, pitch : i64);
}

impl HighlightLensProxy for ObjectRef<HighlightLens> {
    fn set_target(&mut self, target : ObjectHandle) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, ObjectHandle) -> ();
                registry.with_symbol::<FnType, _, _>(
                    "libHighlightLens",
                    "HighlightLens__set_target______obj_mut_dyn_Any____target__ObjectHandle____to__unit__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, target) }
                ).unwrap_or_else(|e| panic!("Failed to call set_target: {}", e))
            } else {
                panic!("Failed to lock object for method set_target")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method set_target"))
    }

    fn set_highlight_color(&mut self, b : u8, g : u8, r : u8, a : u8) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, u8, u8, u8, u8) -> ();
                registry.with_symbol::<FnType, _, _>(
                    "libHighlightLens",
                    "HighlightLens__set_highlight_color______obj_mut_dyn_Any____b__u8____g__u8____r__u8____a__u8____to__unit__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, b, g, r, a) }
                ).unwrap_or_else(|e| panic!("Failed to call set_highlight_color: {}", e))
            } else {
                panic!("Failed to lock object for method set_highlight_color")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method set_highlight_color"))
    }

    fn render(&mut self, buffer : & mut [u8], buffer_width : i64, buffer_height : i64, pitch : i64) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, & mut [u8], i64, i64, i64) -> ();
                registry.with_symbol::<FnType, _, _>(
                    "libHighlightLens",
                    "HighlightLens__render______obj_mut_dyn_Any____buffer__mut_ref_slice_u8____buffer_width__i64____buffer_height__i64____pitch__i64____to__unit__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, buffer, buffer_width, buffer_height, pitch) }
                ).unwrap_or_else(|e| panic!("Failed to call render: {}", e))
            } else {
                panic!("Failed to lock object for method render")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method render"))
    }

}
