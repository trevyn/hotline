use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect as SdlRect;
use std::collections::HashMap;

hotline::object!({
    #[derive(Clone, Debug)]
    pub enum RenderCommand {
        Atlas {
            texture_id: u32,
            src_x: u32,
            src_y: u32,
            src_width: u32,
            src_height: u32,
            dest_x: f64,
            dest_y: f64,
            color: (u8, u8, u8, u8),
        },
    }

    #[derive(Clone, Debug)]
    pub struct AtlasData {
        pub id: u32,
        pub data: Vec<u8>,
        pub width: u32,
        pub height: u32,
        pub format: AtlasFormat,
    }

    #[derive(Clone, Debug)]
    pub enum AtlasFormat {
        GrayscaleAlpha,
        RGBA,
    }

    #[derive(Default)]
    pub struct GPURenderer {
        commands: Vec<RenderCommand>,
        atlases: Vec<AtlasData>,
        next_atlas_id: u32,
    }

    impl GPURenderer {
        pub fn clear_commands(&mut self) {
            self.commands.clear();
        }

        pub fn add_command(&mut self, command: RenderCommand) {
            self.commands.push(command);
        }

        pub fn register_atlas(&mut self, data: Vec<u8>, width: u32, height: u32, format: AtlasFormat) -> u32 {
            let id = self.next_atlas_id;
            self.next_atlas_id += 1;

            self.atlases.push(AtlasData { id, data, width, height, format });

            id
        }

        pub fn commands(&self) -> Vec<RenderCommand> {
            self.commands.clone()
        }

        pub fn atlases(&self) -> Vec<AtlasData> {
            self.atlases.clone()
        }

        pub fn execute_render_internal(
            &mut self,
            canvas: &mut sdl2::render::Canvas<sdl2::video::Window>,
        ) -> Result<(), String> {
            let texture_creator = canvas.texture_creator();

            // Create textures for all atlases
            let mut textures = HashMap::new();
            for atlas in &self.atlases {
                let mut texture = match atlas.format {
                    AtlasFormat::GrayscaleAlpha => texture_creator
                        .create_texture_static(PixelFormatEnum::ABGR8888, atlas.width, atlas.height)
                        .map_err(|e| e.to_string())?,
                    AtlasFormat::RGBA => texture_creator
                        .create_texture_static(PixelFormatEnum::RGBA8888, atlas.width, atlas.height)
                        .map_err(|e| e.to_string())?,
                };

                // Convert atlas data to texture format
                let rgba_data = match atlas.format {
                    AtlasFormat::GrayscaleAlpha => {
                        let mut rgba = vec![0u8; (atlas.width * atlas.height * 4) as usize];
                        for i in 0..(atlas.width * atlas.height) as usize {
                            let _gray = atlas.data[i * 2];
                            let alpha = atlas.data[i * 2 + 1];
                            rgba[i * 4] = alpha; // A
                            rgba[i * 4 + 1] = 255; // B
                            rgba[i * 4 + 2] = 255; // G
                            rgba[i * 4 + 3] = 255; // R
                        }
                        rgba
                    }
                    AtlasFormat::RGBA => atlas.data.clone(),
                };

                texture.update(None, &rgba_data, (atlas.width * 4) as usize).map_err(|e| e.to_string())?;
                textures.insert(atlas.id, texture);
            }

            // Execute render commands
            for command in &self.commands {
                match command {
                    RenderCommand::Atlas { texture_id, src_x, src_y, src_width, src_height, dest_x, dest_y, color } => {
                        if let Some(texture) = textures.get(texture_id) {
                            let src_rect = SdlRect::new(*src_x as i32, *src_y as i32, *src_width, *src_height);

                            let dst_rect = SdlRect::new(*dest_x as i32, *dest_y as i32, *src_width, *src_height);

                            // Apply color modulation
                            canvas.set_draw_color(sdl2::pixels::Color::RGBA(
                                color.2, // R
                                color.1, // G
                                color.0, // B
                                color.3, // A
                            ));

                            canvas.copy(texture, src_rect, dst_rect)?;
                        }
                    }
                }
            }

            Ok(())
        }
    }
});
