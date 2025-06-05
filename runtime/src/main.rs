use bytemuck::{Pod, Zeroable};
use hotline::ObjectHandle;
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use runtime::{DirectRuntime, direct_call};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::mouse::MouseButton;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{TryRecvError, channel};
use std::time::Duration;
use wgpu::util::DeviceExt;
use xxhash_rust::xxh3::xxh3_64;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
}

fn rect_vertices(x: f32, y: f32, w: f32, h: f32, width: f32, height: f32) -> [Vertex; 6] {
    let x1 = x / width * 2.0 - 1.0;
    let y1 = 1.0 - y / height * 2.0;
    let x2 = (x + w) / width * 2.0 - 1.0;
    let y2 = 1.0 - (y + h) / height * 2.0;
    let color = [1.0, 0.0, 0.0, 1.0];
    [
        Vertex { position: [x1, y1], color },
        Vertex { position: [x2, y1], color },
        Vertex { position: [x2, y2], color },
        Vertex { position: [x1, y1], color },
        Vertex { position: [x2, y2], color },
        Vertex { position: [x1, y2], color },
    ]
}

#[cfg(target_os = "linux")]
use png::{BitDepth, ColorType, Encoder};
#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::io::BufWriter;

#[cfg(target_os = "linux")]
fn save_png(path: &str, width: u32, height: u32, data: &[u8]) -> Result<(), String> {
    let file = File::create(path).map_err(|e| e.to_string())?;
    let w = BufWriter::new(file);
    let mut encoder = Encoder::new(w, width, height);
    encoder.set_color(ColorType::Rgba);
    encoder.set_depth(BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(data).map_err(|e| e.to_string())
}

fn main() -> Result<(), String> {
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;

    let window = video_subsystem
        .window("hotline - direct calls", 800, 600)
        .position_centered()
        .vulkan()
        .build()
        .map_err(|e| e.to_string())?;

    let mut event_pump = sdl_context.event_pump()?;

    // Leak the runtime to give it 'static lifetime so objects can store references to it
    let runtime = Box::leak(Box::new({
        #[cfg(target_os = "macos")]
        {
            DirectRuntime::new_with_custom_loader()
        }
        #[cfg(not(target_os = "macos"))]
        {
            DirectRuntime::new()
        }
    }));

    // Dynamically discover and load libraries from objects directory
    use std::fs;
    use std::path::Path;

    let objects_dir = Path::new("objects");
    let mut loaded_libs = Vec::new();

    // First, rebuild all libraries at launch
    // Rebuilding all libraries at launch
    if let Ok(entries) = fs::read_dir(objects_dir) {
        let lib_names: Vec<String> = entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if !path.is_dir() {
                    return None;
                }
                path.file_name()?.to_str().map(|s| s.to_string())
            })
            .collect();

        if !lib_names.is_empty() {
            // Building libraries in parallel
            let mut cmd = std::process::Command::new("cargo");
            cmd.args(&["build", "--release"]);

            // add all packages
            for lib_name in &lib_names {
                cmd.args(&["-p", lib_name]);
            }

            // use status() instead of output() to see real-time output
            let status = cmd.status().expect("failed to build libraries");

            if !status.success() {
                panic!("failed to build libraries");
            }
        }
    }

    // Load all libraries
    if let Ok(entries) = fs::read_dir(objects_dir) {
        let libs_to_load: Vec<(String, String)> = entries
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if !path.is_dir() {
                    return None;
                }
                let lib_name = path.file_name()?.to_str()?.to_string();

                // Construct library path based on OS
                #[cfg(target_os = "macos")]
                let lib_path = format!("target/release/lib{}.dylib", lib_name);
                #[cfg(target_os = "linux")]
                let lib_path = format!("target/release/lib{}.so", lib_name);
                #[cfg(target_os = "windows")]
                let lib_path = format!("target/release/{}.dll", lib_name);

                if Path::new(&lib_path).exists() {
                    Some((lib_name, lib_path))
                } else {
                    eprintln!("Library not found at {}, skipping", lib_path);
                    None
                }
            })
            .collect();

        // Load libraries sequentially (dlopen can be finicky with parallelism)
        // Loading libraries
        let start = std::time::Instant::now();

        for (lib_name, lib_path) in libs_to_load {
            if let Err(e) = runtime.hot_reload(&lib_path, &lib_name) {
                eprintln!("Failed to load {} library: {}", lib_name, e);
            } else {
                loaded_libs.push((lib_name, lib_path));
            }
        }

        let total_elapsed = start.elapsed();
        println!("Total loading time: {:.1}ms", total_elapsed.as_secs_f64() * 1000.0);
    }

    // Store lib paths for hot reload
    let lib_paths = loaded_libs.clone();

    // Set up file watcher for automatic hot reload
    let (tx, rx) = channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default()).expect("Failed to create file watcher");

    // Watch lib.rs files in each object directory and compute initial hashes
    let mut file_hashes: HashMap<String, u64> = HashMap::new();
    for (lib_name, _) in &loaded_libs {
        let lib_rs_path = format!("objects/{}/src/lib.rs", lib_name);
        if Path::new(&lib_rs_path).exists() {
            watcher
                .watch(Path::new(&lib_rs_path), RecursiveMode::NonRecursive)
                .expect(&format!("Failed to watch {}", lib_rs_path));
            // Watching for changes

            // Compute initial hash
            if let Ok(contents) = std::fs::read(&lib_rs_path) {
                let hash = xxh3_64(&contents);
                file_hashes.insert(lib_name.clone(), hash);
            }
        }
    }

    // Create window manager instance
    let window_manager =
        runtime.create_from_lib("libWindowManager", "WindowManager").expect("Failed to create WindowManager");

    // Initialize window manager (which sets up the text renderer)
    direct_call!(runtime, &window_manager, WindowManager, initialize()).expect("Failed to initialize WindowManager");

    // Initialize Vulkan via wgpu
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor { backends: wgpu::Backends::VULKAN, ..Default::default() });
    let surface = unsafe {
        let target = wgpu::SurfaceTargetUnsafe::from_window(&window).map_err(|e| e.to_string())?;
        instance.create_surface_unsafe(target).map_err(|e| e.to_string())?
    };
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .ok_or("adapter")?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&Default::default(), None)).map_err(|e| e.to_string())?;
    let mut config = surface.get_default_config(&adapter, 800, 600).ok_or("config")?;
    surface.configure(&device, &config);

    let shader = device.create_shader_module(wgpu::include_wgsl!("shaders/rect.wgsl"));
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("rect_layout"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });
    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("rect_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            compilation_options: Default::default(),
            buffers: &[wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
            }],
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            compilation_options: Default::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format: config.format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    'running: loop {
        // Check for file system events
        match rx.try_recv() {
            Ok(event) => {
                if let Ok(event) = event {
                    // Find which library changed
                    for (lib_name, lib_path) in &lib_paths {
                        let lib_rs_path = format!("objects/{}/src/lib.rs", lib_name);
                        let lib_rs_pathbuf = PathBuf::from(&lib_rs_path);

                        if event.paths.iter().any(|p| p.ends_with(&lib_rs_pathbuf)) {
                            // Read file and compute hash
                            if let Ok(contents) = std::fs::read(&lib_rs_path) {
                                let new_hash = xxh3_64(&contents);
                                let old_hash = file_hashes.get(lib_name).copied().unwrap_or(0);

                                if new_hash != old_hash {
                                    // Detected change, rebuilding and reloading

                                    // Update hash
                                    file_hashes.insert(lib_name.clone(), new_hash);

                                    // Rebuild the specific library
                                    let status = std::process::Command::new("cargo")
                                        .args(&["build", "--release", "-p", lib_name])
                                        .status()
                                        .expect(&format!("Failed to build {}", lib_name));

                                    if !status.success() {
                                        eprintln!("Failed to build {}", lib_name);
                                        continue;
                                    }

                                    // Reload the library
                                    if let Err(e) = runtime.hot_reload(lib_path, lib_name) {
                                        eprintln!("Failed to reload {} lib: {}", lib_name, e);
                                    } else {
                                        // Successfully reloaded
                                    }
                                }
                            }
                            break;
                        }
                    }
                }
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                eprintln!("File watcher disconnected");
            }
        }

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } | Event::KeyDown { keycode: Some(Keycode::Escape), .. } => break 'running,
                Event::MouseButtonDown { mouse_btn: MouseButton::Left, x, y, .. } => {
                    direct_call!(runtime, &window_manager, WindowManager, handle_mouse_down(x as f64, y as f64))
                        .expect("Failed to handle mouse down");
                }
                Event::MouseButtonUp { mouse_btn: MouseButton::Left, x, y, .. } => {
                    direct_call!(runtime, &window_manager, WindowManager, handle_mouse_up(x as f64, y as f64))
                        .expect("Failed to handle mouse up");
                }
                Event::MouseMotion { x, y, .. } => {
                    direct_call!(runtime, &window_manager, WindowManager, handle_mouse_motion(x as f64, y as f64))
                        .expect("Failed to handle mouse motion");
                }
                Event::KeyDown { keycode: Some(Keycode::R), .. } => {
                    let mut cmd = std::process::Command::new("cargo");
                    cmd.args(&["build", "--release"]);
                    for (lib_name, _) in &lib_paths {
                        cmd.args(&["-p", lib_name]);
                    }
                    let status = cmd.status().expect("Failed to build libraries");
                    if !status.success() {
                        eprintln!("Failed to rebuild libraries");
                        continue;
                    }
                    for (lib_name, lib_path) in &lib_paths {
                        if let Err(e) = runtime.hot_reload(lib_path, lib_name) {
                            eprintln!("Failed to reload {} lib: {}", lib_name, e);
                        }
                    }
                }
                Event::Window { win_event: sdl2::event::WindowEvent::Resized(w, h), .. } => {
                    config.width = w as u32;
                    config.height = h as u32;
                    surface.configure(&device, &config);
                }
                _ => {}
            }
        }

        let output = match surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => {
                surface.configure(&device, &config);
                surface.get_current_texture().expect("frame")
            }
        };
        let view = output.texture.create_view(&Default::default());
        let mut vertices: Vec<Vertex> = Vec::new();
        let rc_any = direct_call!(runtime, &window_manager, WindowManager, get_rects_count()).expect("count");
        let rc = *rc_any.downcast_ref::<i64>().unwrap() as i32;
        for i in 0..rc {
            let h_any = direct_call!(runtime, &window_manager, WindowManager, get_rect_at(i as i64)).expect("rect");
            if let Some(handle) = h_any.downcast_ref::<Option<ObjectHandle>>().unwrap().clone() {
                let b_any = direct_call!(runtime, &handle, Rect, bounds()).expect("bounds");
                let (x, y, w, h) = *b_any.downcast_ref::<(f64, f64, f64, f64)>().unwrap();
                vertices.extend_from_slice(&rect_vertices(
                    x as f32,
                    y as f32,
                    w as f32,
                    h as f32,
                    config.width as f32,
                    config.height as f32,
                ));
            }
        }
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("encoder") });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rpass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.1, g: 0.1, b: 0.1, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            rpass.set_pipeline(&render_pipeline);
            rpass.set_vertex_buffer(0, vertex_buffer.slice(..));
            rpass.draw(0..vertices.len() as u32, 0..1);
        }
        queue.submit(std::iter::once(encoder.finish()));
        output.present();
        ::std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}
