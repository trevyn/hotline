// Auto-generated proxy module for WindowManager
// Include this module in objects that need to call WindowManager methods

use hotline::ObjectRef;

/// Type marker for WindowManager
pub struct WindowManager;

/// Extension methods for ObjectRef<WindowManager>
pub trait WindowManagerProxy {
    fn add_rect(&mut self, rect : ObjectHandle);
    fn clear_selection(&mut self);
    fn set_drag_offset(&mut self, x : f64, y : f64);
    fn start_dragging(&mut self, rect : ObjectHandle);
    fn stop_dragging(&mut self);
    fn get_selected_handle(&mut self) -> Option < ObjectHandle >;
    fn get_rects_count(&mut self) -> i64;
    fn get_rect_at(&mut self, index : i64) -> Option < ObjectHandle >;
    fn is_dragging(&mut self) -> bool;
    fn handle_mouse_down(&mut self, x : f64, y : f64);
    fn handle_mouse_up(&mut self, x : f64, y : f64);
    fn handle_mouse_motion(&mut self, x : f64, y : f64);
    fn render(&mut self, buffer : & mut [u8], buffer_width : i64, buffer_height : i64, pitch : i64);
}

impl WindowManagerProxy for ObjectRef<WindowManager> {
    fn add_rect(&mut self, rect : ObjectHandle) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, ObjectHandle) -> ();
                registry.with_symbol::<FnType, _, _>(
                    "libWindowManager",
                    "WindowManager__add_rect______obj_mut_dyn_Any____rect__ObjectHandle____to__unit__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, rect) }
                ).unwrap_or_else(|e| panic!("Failed to call add_rect: {}", e))
            } else {
                panic!("Failed to lock object for method add_rect")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method add_rect"))
    }

    fn clear_selection(&mut self) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any) -> ();
                registry.with_symbol::<FnType, _, _>(
                    "libWindowManager",
                    "WindowManager__clear_selection______obj_mut_dyn_Any____to__unit__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any) }
                ).unwrap_or_else(|e| panic!("Failed to call clear_selection: {}", e))
            } else {
                panic!("Failed to lock object for method clear_selection")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method clear_selection"))
    }

    fn set_drag_offset(&mut self, x : f64, y : f64) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, f64, f64) -> ();
                registry.with_symbol::<FnType, _, _>(
                    "libWindowManager",
                    "WindowManager__set_drag_offset______obj_mut_dyn_Any____x__f64____y__f64____to__unit__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, x, y) }
                ).unwrap_or_else(|e| panic!("Failed to call set_drag_offset: {}", e))
            } else {
                panic!("Failed to lock object for method set_drag_offset")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method set_drag_offset"))
    }

    fn start_dragging(&mut self, rect : ObjectHandle) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, ObjectHandle) -> ();
                registry.with_symbol::<FnType, _, _>(
                    "libWindowManager",
                    "WindowManager__start_dragging______obj_mut_dyn_Any____rect__ObjectHandle____to__unit__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, rect) }
                ).unwrap_or_else(|e| panic!("Failed to call start_dragging: {}", e))
            } else {
                panic!("Failed to lock object for method start_dragging")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method start_dragging"))
    }

    fn stop_dragging(&mut self) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any) -> ();
                registry.with_symbol::<FnType, _, _>(
                    "libWindowManager",
                    "WindowManager__stop_dragging______obj_mut_dyn_Any____to__unit__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any) }
                ).unwrap_or_else(|e| panic!("Failed to call stop_dragging: {}", e))
            } else {
                panic!("Failed to lock object for method stop_dragging")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method stop_dragging"))
    }

    fn get_selected_handle(&mut self) -> Option < ObjectHandle > {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any) -> Option < ObjectHandle >;
                registry.with_symbol::<FnType, _, _>(
                    "libWindowManager",
                    "WindowManager__get_selected_handle______obj_mut_dyn_Any____to__Option_lt_ObjectHandle_gt__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any) }
                ).unwrap_or_else(|e| panic!("Failed to call get_selected_handle: {}", e))
            } else {
                panic!("Failed to lock object for method get_selected_handle")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method get_selected_handle"))
    }

    fn get_rects_count(&mut self) -> i64 {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any) -> i64;
                registry.with_symbol::<FnType, _, _>(
                    "libWindowManager",
                    "WindowManager__get_rects_count______obj_mut_dyn_Any____to__i64__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any) }
                ).unwrap_or_else(|e| panic!("Failed to call get_rects_count: {}", e))
            } else {
                panic!("Failed to lock object for method get_rects_count")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method get_rects_count"))
    }

    fn get_rect_at(&mut self, index : i64) -> Option < ObjectHandle > {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, i64) -> Option < ObjectHandle >;
                registry.with_symbol::<FnType, _, _>(
                    "libWindowManager",
                    "WindowManager__get_rect_at______obj_mut_dyn_Any____index__i64____to__Option_lt_ObjectHandle_gt__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, index) }
                ).unwrap_or_else(|e| panic!("Failed to call get_rect_at: {}", e))
            } else {
                panic!("Failed to lock object for method get_rect_at")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method get_rect_at"))
    }

    fn is_dragging(&mut self) -> bool {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any) -> bool;
                registry.with_symbol::<FnType, _, _>(
                    "libWindowManager",
                    "WindowManager__is_dragging______obj_mut_dyn_Any____to__bool__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any) }
                ).unwrap_or_else(|e| panic!("Failed to call is_dragging: {}", e))
            } else {
                panic!("Failed to lock object for method is_dragging")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method is_dragging"))
    }

    fn handle_mouse_down(&mut self, x : f64, y : f64) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, f64, f64) -> ();
                registry.with_symbol::<FnType, _, _>(
                    "libWindowManager",
                    "WindowManager__handle_mouse_down______obj_mut_dyn_Any____x__f64____y__f64____to__unit__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, x, y) }
                ).unwrap_or_else(|e| panic!("Failed to call handle_mouse_down: {}", e))
            } else {
                panic!("Failed to lock object for method handle_mouse_down")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method handle_mouse_down"))
    }

    fn handle_mouse_up(&mut self, x : f64, y : f64) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, f64, f64) -> ();
                registry.with_symbol::<FnType, _, _>(
                    "libWindowManager",
                    "WindowManager__handle_mouse_up______obj_mut_dyn_Any____x__f64____y__f64____to__unit__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, x, y) }
                ).unwrap_or_else(|e| panic!("Failed to call handle_mouse_up: {}", e))
            } else {
                panic!("Failed to lock object for method handle_mouse_up")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method handle_mouse_up"))
    }

    fn handle_mouse_motion(&mut self, x : f64, y : f64) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, f64, f64) -> ();
                registry.with_symbol::<FnType, _, _>(
                    "libWindowManager",
                    "WindowManager__handle_mouse_motion______obj_mut_dyn_Any____x__f64____y__f64____to__unit__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, x, y) }
                ).unwrap_or_else(|e| panic!("Failed to call handle_mouse_motion: {}", e))
            } else {
                panic!("Failed to lock object for method handle_mouse_motion")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method handle_mouse_motion"))
    }

    fn render(&mut self, buffer : & mut [u8], buffer_width : i64, buffer_height : i64, pitch : i64) {
        crate::with_library_registry(|registry| {
            if let Ok(mut guard) = self.inner().lock() {
                let obj_any = guard.as_any_mut();
                type FnType = unsafe extern "Rust" fn(&mut dyn std::any::Any, & mut [u8], i64, i64, i64) -> ();
                registry.with_symbol::<FnType, _, _>(
                    "libWindowManager",
                    "WindowManager__render______obj_mut_dyn_Any____buffer__mut_ref_slice_u8____buffer_width__i64____buffer_height__i64____pitch__i64____to__unit__5d707b07e",
                    |fn_ptr| unsafe { (**fn_ptr)(obj_any, buffer, buffer_width, buffer_height, pitch) }
                ).unwrap_or_else(|e| panic!("Failed to call render: {}", e))
            } else {
                panic!("Failed to lock object for method render")
            }
        }).unwrap_or_else(|| panic!("No library registry available for method render"))
    }

}
