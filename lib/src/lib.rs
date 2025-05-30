#[derive(Debug, Clone)]
pub struct Box {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[no_mangle]
pub fn create_box(x: i32, y: i32, width: u32, height: u32) -> Box {
    Box {
        x,
        y,
        width,
        height,
    }
}

#[no_mangle]
pub fn render_box(b: &Box, surface_data: &mut [u8], surface_width: u32, surface_height: u32) {
    let bytes_per_pixel = 4;
    let pitch = surface_width * bytes_per_pixel;

    let x_start = b.x.max(0) as u32;
    let y_start = b.y.max(0) as u32;
    let x_end = ((b.x + b.width as i32).min(surface_width as i32) as u32).min(surface_width);
    let y_end = ((b.y + b.height as i32).min(surface_height as i32) as u32).min(surface_height);

    for y in y_start..y_end {
        for x in x_start..x_end {
            let offset = (y * pitch + x * bytes_per_pixel) as usize;
            if offset + 3 < surface_data.len() {
                surface_data[offset] = 255; // B
                surface_data[offset + 1] = 0; // G
                surface_data[offset + 2] = 0; // R
                surface_data[offset + 3] = 255; // A
            }
        }
    }
}
