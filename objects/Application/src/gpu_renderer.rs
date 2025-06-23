use sdl3::gpu::{
    BlendFactor, BlendOp, Buffer, BufferBinding, BufferUsageFlags, ColorTargetBlendState, ColorTargetDescription,
    ColorTargetInfo, CommandBuffer, Device, FillMode, Filter, GraphicsPipeline, GraphicsPipelineTargetInfo, LoadOp,
    PrimitiveType, Sampler, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode, ShaderFormat, ShaderStage,
    StoreOp, Texture, TextureCreateInfo, TextureFormat, TextureSamplerBinding, TextureType, TextureUsage,
    TransferBuffer, TransferBufferUsage, VertexAttribute, VertexBufferDescription, VertexElementFormat,
    VertexInputRate, VertexInputState,
};
use std::collections::HashMap;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct QuadVertex {
    pub pos: [f32; 2],
    pub tex_coord: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct SolidVertex {
    pub pos: [f32; 2],
    pub color: [f32; 4],
}

// Wrapper to make GpuRenderer thread-safe for Hotline
pub struct GpuRenderer {
    // Store as raw pointer to work around Send+Sync requirements
    inner: *mut GpuRendererInner,
}

// Mark as Send+Sync - we ensure thread safety by only accessing from main thread
unsafe impl Send for GpuRenderer {}
unsafe impl Sync for GpuRenderer {}

// Implement Clone - this is a no-op since we can't actually clone the GPU device
impl Clone for GpuRenderer {
    fn clone(&self) -> Self {
        panic!("GpuRenderer cannot be cloned - GPU resources are not clonable")
    }
}

impl Default for GpuRenderer {
    fn default() -> Self {
        Self { inner: std::ptr::null_mut() }
    }
}

impl Drop for GpuRenderer {
    fn drop(&mut self) {
        unsafe {
            if !self.inner.is_null() {
                let _ = Box::from_raw(self.inner);
            }
        }
    }
}

struct GpuRendererInner {
    device: Device,
    quad_pipeline: GraphicsPipeline,
    solid_pipeline: GraphicsPipeline,
    sampler: Sampler,
    textures: HashMap<u32, Texture<'static>>,
    white_texture: Texture<'static>,
    transfer_buffer: TransferBuffer,
    quad_vertex_buffer: Buffer,
    solid_vertex_buffer: Buffer,
    quad_vertices: Vec<QuadVertex>,
    solid_vertices: Vec<SolidVertex>,
    next_texture_id: u32,
    // Track texture batches: texture_id -> (start_index, count)
    texture_batches: Vec<(u32, usize, usize)>,
}

impl ::hotline::GpuRenderingContext for GpuRenderer {
    fn create_rgba_texture(&mut self, data: &[u8], width: u32, height: u32) -> Result<u32, String> {
        self.create_rgba_texture(data, width, height)
    }

    fn add_textured_rect(&mut self, x: f32, y: f32, w: f32, h: f32, tex_id: u32, color: [f32; 4]) {
        self.add_textured_rect(x, y, w, h, tex_id, color);
    }

    fn add_textured_rect_with_coords(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex_id: u32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        color: [f32; 4],
    ) {
        self.add_textured_rect_with_coords(x, y, w, h, tex_id, u0, v0, u1, v1, color);
    }

    fn add_solid_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.add_solid_rect(x, y, w, h, color);
    }

    fn add_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32, color: [f32; 4]) {
        self.add_line(x1, y1, x2, y2, thickness, color);
    }
}

impl GpuRenderer {
    fn inner(&self) -> &GpuRendererInner {
        if self.inner.is_null() {
            panic!("GpuRenderer not initialized")
        }
        unsafe { &*self.inner }
    }

    fn inner_mut(&mut self) -> &mut GpuRendererInner {
        if self.inner.is_null() {
            panic!("GpuRenderer not initialized")
        }
        unsafe { &mut *self.inner }
    }

    // Add lock method for object macro compatibility
    pub fn lock(&self) -> &Self {
        self
    }

    pub fn new(window: &sdl3::video::Window) -> Result<Self, String> {
        // Try to create GPU device
        // Use SpirV format since that's what our shaders are compiled to
        let device = match Device::new(
            ShaderFormat::SpirV,
            false, // disable debug mode to avoid potential issues
        ) {
            Ok(d) => d,
            Err(e) => {
                return Err(format!("Failed to create GPU device: {}", e));
            }
        };

        let device = device.with_window(window).map_err(|e| e.to_string())?;

        // Disable vsync for performance testing
        unsafe {
            use sdl3::sys;
            if !sys::gpu::SDL_SetGPUSwapchainParameters(
                device.raw(),
                window.raw(),
                sys::gpu::SDL_GPUSwapchainComposition::SDR,
                sys::gpu::SDL_GPUPresentMode::IMMEDIATE,
            ) {
                eprintln!("Warning: Failed to disable vsync");
            } else {
                eprintln!("Vsync disabled (IMMEDIATE present mode)");
            }
        }

        // Load shaders
        let quad_vs = include_bytes!(concat!(env!("OUT_DIR"), "/shaders/quad.vert.spv"));
        let quad_fs = include_bytes!(concat!(env!("OUT_DIR"), "/shaders/quad.frag.spv"));
        let solid_vs = include_bytes!(concat!(env!("OUT_DIR"), "/shaders/solid.vert.spv"));
        let solid_fs = include_bytes!(concat!(env!("OUT_DIR"), "/shaders/solid.frag.spv"));

        // Create shaders
        let quad_vs_shader = device
            .create_shader()
            .with_code(ShaderFormat::SpirV, quad_vs, ShaderStage::Vertex)
            .with_uniform_buffers(1)
            .with_entrypoint(c"main")
            .build()
            .map_err(|e| e.to_string())?;

        let quad_fs_shader = device
            .create_shader()
            .with_code(ShaderFormat::SpirV, quad_fs, ShaderStage::Fragment)
            .with_samplers(1)
            .with_entrypoint(c"main")
            .build()
            .map_err(|e| e.to_string())?;

        let solid_vs_shader = device
            .create_shader()
            .with_code(ShaderFormat::SpirV, solid_vs, ShaderStage::Vertex)
            .with_uniform_buffers(1)
            .with_entrypoint(c"main")
            .build()
            .map_err(|e| e.to_string())?;

        let solid_fs_shader = device
            .create_shader()
            .with_code(ShaderFormat::SpirV, solid_fs, ShaderStage::Fragment)
            .with_entrypoint(c"main")
            .build()
            .map_err(|e| e.to_string())?;

        let swapchain_format = device.get_swapchain_texture_format(window);

        // Create quad pipeline (for textured rendering)
        let quad_pipeline = device
            .create_graphics_pipeline()
            .with_vertex_shader(&quad_vs_shader)
            .with_fragment_shader(&quad_fs_shader)
            .with_primitive_type(PrimitiveType::TriangleList)
            .with_fill_mode(FillMode::Fill)
            .with_vertex_input_state(
                VertexInputState::new()
                    .with_vertex_buffer_descriptions(&[VertexBufferDescription::new()
                        .with_slot(0)
                        .with_pitch(std::mem::size_of::<QuadVertex>() as u32)
                        .with_input_rate(VertexInputRate::Vertex)])
                    .with_vertex_attributes(&[
                        VertexAttribute::new()
                            .with_format(VertexElementFormat::Float2)
                            .with_location(0)
                            .with_buffer_slot(0)
                            .with_offset(0),
                        VertexAttribute::new()
                            .with_format(VertexElementFormat::Float2)
                            .with_location(1)
                            .with_buffer_slot(0)
                            .with_offset(8),
                        VertexAttribute::new()
                            .with_format(VertexElementFormat::Float4)
                            .with_location(2)
                            .with_buffer_slot(0)
                            .with_offset(16),
                    ]),
            )
            .with_target_info(
                GraphicsPipelineTargetInfo::new().with_color_target_descriptions(&[ColorTargetDescription::new()
                    .with_format(swapchain_format)
                    .with_blend_state(
                        ColorTargetBlendState::new()
                            .with_enable_blend(true)
                            .with_src_color_blendfactor(BlendFactor::SrcAlpha)
                            .with_dst_color_blendfactor(BlendFactor::OneMinusSrcAlpha)
                            .with_color_blend_op(BlendOp::Add)
                            .with_src_alpha_blendfactor(BlendFactor::SrcAlpha)
                            .with_dst_alpha_blendfactor(BlendFactor::OneMinusSrcAlpha)
                            .with_alpha_blend_op(BlendOp::Add),
                    )]),
            )
            .build()
            .map_err(|e| {
                eprintln!("Failed to create quad pipeline: {}", e);
                e.to_string()
            })?;

        eprintln!("Quad pipeline created successfully");

        // Create solid pipeline (for non-textured rendering)
        let solid_pipeline = device
            .create_graphics_pipeline()
            .with_vertex_shader(&solid_vs_shader)
            .with_fragment_shader(&solid_fs_shader)
            .with_primitive_type(PrimitiveType::TriangleList)
            .with_fill_mode(FillMode::Fill)
            .with_vertex_input_state(
                VertexInputState::new()
                    .with_vertex_buffer_descriptions(&[VertexBufferDescription::new()
                        .with_slot(0)
                        .with_pitch(std::mem::size_of::<SolidVertex>() as u32)
                        .with_input_rate(VertexInputRate::Vertex)])
                    .with_vertex_attributes(&[
                        VertexAttribute::new()
                            .with_format(VertexElementFormat::Float2)
                            .with_location(0)
                            .with_buffer_slot(0)
                            .with_offset(0), // pos: [f32; 2] at offset 0
                        VertexAttribute::new()
                            .with_format(VertexElementFormat::Float4)
                            .with_location(1)
                            .with_buffer_slot(0)
                            .with_offset(8), // color: [f32; 4] at offset 8 (after 2 * f32)
                    ]),
            )
            .with_target_info(
                GraphicsPipelineTargetInfo::new().with_color_target_descriptions(&[ColorTargetDescription::new()
                    .with_format(swapchain_format)
                    .with_blend_state(
                        ColorTargetBlendState::new()
                            .with_enable_blend(true)
                            .with_src_color_blendfactor(BlendFactor::SrcAlpha)
                            .with_dst_color_blendfactor(BlendFactor::OneMinusSrcAlpha)
                            .with_color_blend_op(BlendOp::Add)
                            .with_src_alpha_blendfactor(BlendFactor::SrcAlpha)
                            .with_dst_alpha_blendfactor(BlendFactor::OneMinusSrcAlpha)
                            .with_alpha_blend_op(BlendOp::Add),
                    )]),
            )
            .build()
            .map_err(|e| {
                eprintln!("Failed to create solid pipeline: {}", e);
                e.to_string()
            })?;

        eprintln!("Solid pipeline created successfully");

        // Create sampler
        let sampler = device
            .create_sampler(
                SamplerCreateInfo::new()
                    .with_min_filter(Filter::Nearest)
                    .with_mag_filter(Filter::Nearest)
                    .with_mipmap_mode(SamplerMipmapMode::Nearest)
                    .with_address_mode_u(SamplerAddressMode::ClampToEdge)
                    .with_address_mode_v(SamplerAddressMode::ClampToEdge),
            )
            .map_err(|e| e.to_string())?;

        // Create white pixel texture
        let white_texture = device
            .create_texture(
                TextureCreateInfo::new()
                    .with_type(TextureType::_2D)
                    .with_format(TextureFormat::R8g8b8a8Unorm)
                    .with_width(1)
                    .with_height(1)
                    .with_layer_count_or_depth(1)
                    .with_num_levels(1)
                    .with_usage(TextureUsage::Sampler),
            )
            .map_err(|e| e.to_string())?;

        // Create transfer buffer
        let transfer_buffer = device
            .create_transfer_buffer()
            .with_size(32 * 1024 * 1024) // 32MB transfer buffer
            .with_usage(TransferBufferUsage::Upload)
            .build()
            .map_err(|e| e.to_string())?;

        // Upload white pixel
        let white_pixel: [u8; 4] = [255, 255, 255, 255];
        let cmd = device.acquire_command_buffer().map_err(|e| e.to_string())?;

        // Map transfer buffer and copy data
        let mut map = transfer_buffer.map::<u8>(&device, false);
        map.mem_mut()[..4].copy_from_slice(&white_pixel);
        map.unmap();

        let copy_pass = device.begin_copy_pass(&cmd).map_err(|e| e.to_string())?;
        copy_pass.upload_to_gpu_texture(
            sdl3::gpu::TextureTransferInfo::new().with_transfer_buffer(&transfer_buffer).with_offset(0),
            sdl3::gpu::TextureRegion::new().with_texture(&white_texture).with_width(1).with_height(1).with_depth(1),
            false,
        );
        device.end_copy_pass(copy_pass);
        cmd.submit().map_err(|e| e.to_string())?;

        // Create vertex buffers
        let solid_vertex_buffer = device
            .create_buffer()
            .with_size(10000 * std::mem::size_of::<SolidVertex>() as u32)
            .with_usage(BufferUsageFlags::Vertex)
            .build()
            .map_err(|e| e.to_string())?;

        let quad_vertex_buffer = device
            .create_buffer()
            .with_size(10000 * std::mem::size_of::<QuadVertex>() as u32)
            .with_usage(BufferUsageFlags::Vertex)
            .build()
            .map_err(|e| e.to_string())?;

        let inner = Box::new(GpuRendererInner {
            device,
            quad_pipeline,
            solid_pipeline,
            sampler,
            textures: HashMap::new(),
            white_texture,
            transfer_buffer,
            quad_vertex_buffer,
            solid_vertex_buffer,
            quad_vertices: Vec::new(),
            solid_vertices: Vec::new(),
            next_texture_id: 1,
            texture_batches: Vec::new(),
        });

        Ok(Self { inner: Box::into_raw(inner) })
    }

    pub fn acquire_command_buffer(&self) -> Result<CommandBuffer, String> {
        self.inner().device.acquire_command_buffer().map_err(|e| e.to_string())
    }

    pub fn create_texture(
        &mut self,
        data: &[u8],
        width: u32,
        height: u32,
        format: TextureFormat,
    ) -> Result<u32, String> {
        self.create_texture_internal(data, width, height, format)
    }

    pub fn create_rgba_texture(&mut self, data: &[u8], width: u32, height: u32) -> Result<u32, String> {
        self.create_texture_internal(data, width, height, TextureFormat::R8g8b8a8Unorm)
    }

    fn create_texture_internal(
        &mut self,
        data: &[u8],
        width: u32,
        height: u32,
        format: TextureFormat,
    ) -> Result<u32, String> {
        let inner = self.inner_mut();
        let id = inner.next_texture_id;
        inner.next_texture_id += 1;

        let texture = inner
            .device
            .create_texture(
                TextureCreateInfo::new()
                    .with_type(TextureType::_2D)
                    .with_format(format)
                    .with_width(width)
                    .with_height(height)
                    .with_layer_count_or_depth(1)
                    .with_num_levels(1)
                    .with_usage(TextureUsage::Sampler),
            )
            .map_err(|e| e.to_string())?;

        // Upload texture data
        let cmd = inner.device.acquire_command_buffer().map_err(|e| e.to_string())?;

        // Map transfer buffer and copy data
        let mut map = inner.transfer_buffer.map::<u8>(&inner.device, false);
        map.mem_mut()[..data.len()].copy_from_slice(data);
        map.unmap();

        let copy_pass = inner.device.begin_copy_pass(&cmd).map_err(|e| e.to_string())?;
        copy_pass.upload_to_gpu_texture(
            sdl3::gpu::TextureTransferInfo::new().with_transfer_buffer(&inner.transfer_buffer).with_offset(0),
            sdl3::gpu::TextureRegion::new().with_texture(&texture).with_width(width).with_height(height).with_depth(1),
            false,
        );
        inner.device.end_copy_pass(copy_pass);
        cmd.submit().map_err(|e| e.to_string())?;

        inner.textures.insert(id, texture);
        Ok(id)
    }

    pub fn begin_frame(&mut self) {
        let inner = self.inner_mut();
        inner.quad_vertices.clear();
        inner.solid_vertices.clear();
        inner.texture_batches.clear();
    }

    pub fn add_textured_rect(&mut self, x: f32, y: f32, w: f32, h: f32, tex_id: u32, color: [f32; 4]) {
        self.add_textured_rect_with_coords(x, y, w, h, tex_id, 0.0, 0.0, 1.0, 1.0, color);
    }

    pub fn add_textured_rect_with_coords(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        tex_id: u32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        color: [f32; 4],
    ) {
        let vertices = [
            QuadVertex { pos: [x, y], tex_coord: [u0, v0], color },
            QuadVertex { pos: [x + w, y], tex_coord: [u1, v0], color },
            QuadVertex { pos: [x, y + h], tex_coord: [u0, v1], color },
            QuadVertex { pos: [x + w, y], tex_coord: [u1, v0], color },
            QuadVertex { pos: [x + w, y + h], tex_coord: [u1, v1], color },
            QuadVertex { pos: [x, y + h], tex_coord: [u0, v1], color },
        ];

        let inner = self.inner_mut();
        let start_index = inner.quad_vertices.len();
        inner.quad_vertices.extend_from_slice(&vertices);

        // Update or create batch for this texture
        if let Some(batch) = inner.texture_batches.last_mut() {
            if batch.0 == tex_id {
                // Add to existing batch
                batch.2 += 6; // 6 vertices per quad
            } else {
                // Start new batch
                inner.texture_batches.push((tex_id, start_index, 6));
            }
        } else {
            // First batch
            inner.texture_batches.push((tex_id, start_index, 6));
        }
    }

    pub fn add_solid_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        // Debug suspicious rectangles near origin
        static mut RECT_COUNT: u32 = 0;
        unsafe {
            if RECT_COUNT < 5 && (x.abs() < 50.0 && y.abs() < 50.0) {
                eprintln!("GpuRenderer: Solid rect near origin at ({}, {}) size {}x{}", x, y, w, h);
                RECT_COUNT += 1;
            }
        }

        // Don't add degenerate rectangles
        if w <= 0.0 || h <= 0.0 {
            return; // Silently reject
        }

        let vertices = [
            SolidVertex { pos: [x, y], color },
            SolidVertex { pos: [x + w, y], color },
            SolidVertex { pos: [x, y + h], color },
            SolidVertex { pos: [x + w, y], color },
            SolidVertex { pos: [x + w, y + h], color },
            SolidVertex { pos: [x, y + h], color },
        ];
        self.inner_mut().solid_vertices.extend_from_slice(&vertices);
    }

    pub fn add_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, thickness: f32, color: [f32; 4]) {
        // Debug log lines near origin
        static mut LINE_COUNT: u32 = 0;
        unsafe {
            if LINE_COUNT < 10 && ((x1.abs() < 200.0 && y1.abs() < 200.0) || (x2.abs() < 200.0 && y2.abs() < 200.0)) {
                eprintln!("Line near origin: ({}, {}) to ({}, {}), thickness={}", x1, y1, x2, y2, thickness);
                LINE_COUNT += 1;
            }
        }

        // Reject lines that have an endpoint very close to origin
        // This is a defensive check to prevent spurious lines
        const ORIGIN_THRESHOLD: f32 = 100.0;
        if (x1.abs() < ORIGIN_THRESHOLD && y1.abs() < ORIGIN_THRESHOLD)
            || (x2.abs() < ORIGIN_THRESHOLD && y2.abs() < ORIGIN_THRESHOLD)
        {
            eprintln!("REJECTED line due to origin proximity");
            return; // Silently reject
        }

        // Calculate perpendicular vector for line thickness
        let dx = x2 - x1;
        let dy = y2 - y1;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.001 {
            eprintln!("REJECTED degenerate line: len={}", len);
            return;
        }

        let px = -dy / len * thickness / 2.0;
        let py = dx / len * thickness / 2.0;

        // Create a quad for the line
        let vertices = [
            SolidVertex { pos: [x1 - px, y1 - py], color },
            SolidVertex { pos: [x2 - px, y2 - py], color },
            SolidVertex { pos: [x1 + px, y1 + py], color },
            SolidVertex { pos: [x2 - px, y2 - py], color },
            SolidVertex { pos: [x2 + px, y2 + py], color },
            SolidVertex { pos: [x1 + px, y1 + py], color },
        ];
        self.inner_mut().solid_vertices.extend_from_slice(&vertices);
    }

    pub fn render_frame(&mut self, window: &sdl3::video::Window) -> Result<(), String> {
        let inner = self.inner_mut();

        // Upload vertex data if we have any
        if !inner.solid_vertices.is_empty() {
            let solid_data = unsafe {
                std::slice::from_raw_parts(
                    inner.solid_vertices.as_ptr() as *const u8,
                    inner.solid_vertices.len() * std::mem::size_of::<SolidVertex>(),
                )
            };

            // Create a command buffer for copy operations
            let copy_cmd = inner.device.acquire_command_buffer().map_err(|e| e.to_string())?;

            // Create a copy pass to upload vertex data
            let copy_pass = inner.device.begin_copy_pass(&copy_cmd).map_err(|e| e.to_string())?;

            // Map transfer buffer and copy data
            // Check if data fits in transfer buffer
            if solid_data.len() > 32 * 1024 * 1024 {
                eprintln!("Warning: Solid data size {} exceeds transfer buffer size, skipping", solid_data.len());
                return Ok(());
            }

            // Use cycle=true for proper synchronization when updating buffers frequently
            let mut map = inner.transfer_buffer.map::<u8>(&inner.device, true);
            map.mem_mut()[..solid_data.len()].copy_from_slice(solid_data);
            map.unmap();

            copy_pass.upload_to_gpu_buffer(
                sdl3::gpu::TransferBufferLocation::new().with_transfer_buffer(&inner.transfer_buffer).with_offset(0),
                sdl3::gpu::BufferRegion::new()
                    .with_buffer(&inner.solid_vertex_buffer)
                    .with_offset(0)
                    .with_size(solid_data.len() as u32),
                true, // cycle=true for synchronization
            );
            inner.device.end_copy_pass(copy_pass);

            // Submit the copy command buffer and wait for completion
            copy_cmd.submit().map_err(|e| e.to_string())?;
        }

        // Acquire command buffer for rendering
        let mut command_buffer = inner.device.acquire_command_buffer().map_err(|e| e.to_string())?;

        // Get raw pointer to command buffer to work around lifetime issues
        let cmd_ptr = &mut command_buffer as *mut CommandBuffer;

        // Acquire swapchain texture
        if let Ok(swapchain) = unsafe { (*cmd_ptr).wait_and_acquire_swapchain_texture(window) } {
            // Begin render pass
            let render_pass = inner
                .device
                .begin_render_pass(
                    unsafe { &*cmd_ptr },
                    &[ColorTargetInfo::default()
                        .with_texture(&swapchain)
                        .with_load_op(LoadOp::Clear)
                        .with_store_op(StoreOp::Store)
                        .with_clear_color(sdl3::pixels::Color::RGBA(50, 50, 50, 255))], // Dark gray with full alpha
                    None,
                )
                .map_err(|e| e.to_string())?;

            // Render solid colored geometry if we have any
            if !inner.solid_vertices.is_empty() {
                // Bind pipeline and vertex buffer
                render_pass.bind_graphics_pipeline(&inner.solid_pipeline);
                render_pass.bind_vertex_buffers(
                    0,
                    &[BufferBinding::new().with_buffer(&inner.solid_vertex_buffer).with_offset(0)],
                );

                // Set push constants after binding pipeline
                let (screen_width, screen_height) = window.size();
                let push_constants = [screen_width as f32, screen_height as f32];

                // Debug screen size
                static mut LOGGED_SIZE: bool = false;
                unsafe {
                    if !LOGGED_SIZE {
                        eprintln!("Solid pipeline: screen_size = {}x{}", screen_width, screen_height);
                        LOGGED_SIZE = true;
                    }
                    (*cmd_ptr).push_vertex_uniform_data(0, &push_constants);
                }

                // Draw all the vertices
                let vertex_count = inner.solid_vertices.len();

                // Ensure vertex count is a multiple of 3 (complete triangles)
                if vertex_count % 3 != 0 {
                    eprintln!("WARNING: Solid vertex count {} is not a multiple of 3!", vertex_count);
                }

                // draw_primitives expects the number of vertices
                render_pass.draw_primitives(vertex_count, 1, 0, 0);
            }

            // Render textured geometry in batches
            if !inner.quad_vertices.is_empty() {
                // Upload quad vertex data
                let quad_data = unsafe {
                    std::slice::from_raw_parts(
                        inner.quad_vertices.as_ptr() as *const u8,
                        inner.quad_vertices.len() * std::mem::size_of::<QuadVertex>(),
                    )
                };

                // Create a command buffer for copy operations
                let copy_cmd = inner.device.acquire_command_buffer().map_err(|e| e.to_string())?;

                // Create a copy pass to upload vertex data
                let copy_pass = inner.device.begin_copy_pass(&copy_cmd).map_err(|e| e.to_string())?;

                // Map transfer buffer and copy data
                // Use cycle=true for proper synchronization when updating buffers frequently
                // Check if data fits in transfer buffer
                if quad_data.len() > 32 * 1024 * 1024 {
                    eprintln!(
                        "Warning: Quad data size {} bytes ({} vertices) exceeds transfer buffer size, skipping",
                        quad_data.len(),
                        inner.quad_vertices.len()
                    );
                    return Ok(());
                }

                let mut map = inner.transfer_buffer.map::<u8>(&inner.device, true);
                let mem = map.mem_mut();

                // Copy our data
                mem[..quad_data.len()].copy_from_slice(quad_data);
                map.unmap();

                copy_pass.upload_to_gpu_buffer(
                    sdl3::gpu::TransferBufferLocation::new()
                        .with_transfer_buffer(&inner.transfer_buffer)
                        .with_offset(0),
                    sdl3::gpu::BufferRegion::new()
                        .with_buffer(&inner.quad_vertex_buffer)
                        .with_offset(0)
                        .with_size(quad_data.len() as u32),
                    true, // cycle=true for synchronization
                );
                inner.device.end_copy_pass(copy_pass);

                // Submit the copy command buffer and wait for it
                copy_cmd.submit().map_err(|e| e.to_string())?;

                // IMPORTANT: We need to ensure the copy completes before rendering
                // This might be the source of our garbage data issues

                // Bind pipeline and vertex buffer once
                render_pass.bind_graphics_pipeline(&inner.quad_pipeline);
                render_pass.bind_vertex_buffers(
                    0,
                    &[BufferBinding::new().with_buffer(&inner.quad_vertex_buffer).with_offset(0)],
                );

                // Get screen size for push constants
                let (screen_width, screen_height) = window.size();
                let push_constants = [screen_width as f32, screen_height as f32];

                // Set push constants once
                unsafe {
                    (*cmd_ptr).push_vertex_uniform_data(0, &push_constants);
                }

                // Render each texture batch
                for (batch_idx, &(tex_id, start_index, count)) in inner.texture_batches.iter().enumerate() {
                    // Validate we're not drawing past our vertex data
                    if start_index + count > inner.quad_vertices.len() {
                        eprintln!(
                            "ERROR: Batch {} would draw past vertices! tex_id={}, start={}, count={}, len={}",
                            batch_idx,
                            tex_id,
                            start_index,
                            count,
                            inner.quad_vertices.len()
                        );
                        continue;
                    }

                    // Get the texture or use white texture as fallback
                    let texture = if let Some(tex) = inner.textures.get(&tex_id) { tex } else { &inner.white_texture };

                    // Bind texture for this batch
                    render_pass.bind_fragment_samplers(
                        0,
                        &[TextureSamplerBinding::new().with_sampler(&inner.sampler).with_texture(texture)],
                    );

                    // Draw this batch
                    render_pass.draw_primitives(count, 1, start_index, 0);
                }
            }

            inner.device.end_render_pass(render_pass);

            // Submit the command buffer
            command_buffer.submit().map_err(|e| e.to_string())?;
        } else {
            // Swapchain unavailable, cancel work
            command_buffer.cancel();
        }

        Ok(())
    }
}
