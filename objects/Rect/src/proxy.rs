// Auto-generated proxy module for Rect
// Include this module in objects that need to call Rect methods

use hotline::ObjectRef;

/// Type marker for Rect
pub struct Rect;

/// Extension methods for ObjectRef<Rect>
pub trait RectProxy {
    fn initialize(&mut self, x : f64, y : f64, width : f64, height : f64);
    fn contains_point(&mut self, point_x : f64, point_y : f64) -> bool;
    fn position(&mut self) -> (f64, f64);
    fn bounds(&mut self) -> (f64, f64, f64, f64);
    fn move_by(&mut self, dx : f64, dy : f64);
    fn render(&mut self, buffer : & mut [u8], buffer_width : i64, buffer_height : i64, pitch : i64);
}

impl RectProxy for ObjectRef<Rect> {
    fn initialize(&mut self, x : f64, y : f64, width : f64, height : f64) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, f64, f64, f64, f64) -> ();
                registry.with_symbol::<FnType, _, _>(
                    "libRect",
                    "Rect__initialize______obj_mut_dyn_Any____x__f64____y__f64____width__f64____height__f64____to__unit__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, x, y, width, height) }
                ).unwrap_or_else(|e| panic!("Failed to call initialize: {}", e))
            } else {
                panic!("Failed to lock object for method initialize")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method initialize"))
    }

    fn contains_point(&mut self, point_x : f64, point_y : f64) -> bool {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, f64, f64) -> bool;
                registry.with_symbol::<FnType, _, _>(
                    "libRect",
                    "Rect__contains_point______obj_mut_dyn_Any____point_x__f64____point_y__f64____to__bool__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, point_x, point_y) }
                ).unwrap_or_else(|e| panic!("Failed to call contains_point: {}", e))
            } else {
                panic!("Failed to lock object for method contains_point")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method contains_point"))
    }

    fn position(&mut self) -> (f64, f64) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any) -> (f64, f64);
                registry.with_symbol::<FnType, _, _>(
                    "libRect",
                    "Rect__position______obj_mut_dyn_Any____to__tuple_f64_comma_f64__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any) }
                ).unwrap_or_else(|e| panic!("Failed to call position: {}", e))
            } else {
                panic!("Failed to lock object for method position")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method position"))
    }

    fn bounds(&mut self) -> (f64, f64, f64, f64) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any) -> (f64, f64, f64, f64);
                registry.with_symbol::<FnType, _, _>(
                    "libRect",
                    "Rect__bounds______obj_mut_dyn_Any____to__tuple_f64_comma_f64_comma_f64_comma_f64__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any) }
                ).unwrap_or_else(|e| panic!("Failed to call bounds: {}", e))
            } else {
                panic!("Failed to lock object for method bounds")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method bounds"))
    }

    fn move_by(&mut self, dx : f64, dy : f64) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, f64, f64) -> ();
                registry.with_symbol::<FnType, _, _>(
                    "libRect",
                    "Rect__move_by______obj_mut_dyn_Any____dx__f64____dy__f64____to__unit__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, dx, dy) }
                ).unwrap_or_else(|e| panic!("Failed to call move_by: {}", e))
            } else {
                panic!("Failed to lock object for method move_by")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method move_by"))
    }

    fn render(&mut self, buffer : & mut [u8], buffer_width : i64, buffer_height : i64, pitch : i64) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, & mut [u8], i64, i64, i64) -> ();
                registry.with_symbol::<FnType, _, _>(
                    "libRect",
                    "Rect__render______obj_mut_dyn_Any____buffer__mut_ref_slice_u8____buffer_width__i64____buffer_height__i64____pitch__i64____to__unit__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, buffer, buffer_width, buffer_height, pitch) }
                ).unwrap_or_else(|e| panic!("Failed to call render: {}", e))
            } else {
                panic!("Failed to lock object for method render")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method render"))
    }

}
