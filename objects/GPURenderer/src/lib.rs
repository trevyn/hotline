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

        pub fn render_via(&self, app: &mut Like<Application>) -> Result<(), String> {
            // Send all atlases to application
            for atlas in &self.atlases {
                app.gpu_receive_atlas(atlas.clone())?;
            }

            // Send all render commands to application
            for command in &self.commands {
                app.gpu_receive_command(command.clone())?;
            }

            Ok(())
        }
    }
});
