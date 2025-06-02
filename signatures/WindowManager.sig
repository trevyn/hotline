add_rect:ObjectHandle:()
clear_selection::()
set_drag_offset:f64,f64:()
start_dragging:ObjectHandle:()
stop_dragging::()
get_selected_handle::Option < ObjectHandle >
get_rects_count::i64
get_rect_at:i64:Option < ObjectHandle >
is_dragging::bool
handle_mouse_down:f64,f64:()
handle_mouse_up:f64,f64:()
handle_mouse_motion:f64,f64:()
render:mut_ref_slice_u8,i64,i64,i64:()