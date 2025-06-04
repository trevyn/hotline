hotline::object!({
    #[derive(Clone, Default)]
    pub struct PNGLoader {
        loaded_data: Option<(Vec<u8>, u32, u32)>, // (data, width, height)
    }

    impl PNGLoader {
        pub fn load_png(&mut self, path: &str) -> Result<(), String> {
            let png_data = std::fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;

            let decoder = png::Decoder::new(png_data.as_slice());
            let mut reader = decoder.read_info().map_err(|e| format!("Failed to read PNG info: {}", e))?;

            let mut buffer = vec![0u8; reader.output_buffer_size()];
            reader.next_frame(&mut buffer).map_err(|e| format!("Failed to decode PNG: {}", e))?;

            let width = reader.info().width;
            let height = reader.info().height;

            self.loaded_data = Some((buffer, width, height));
            Ok(())
        }

        pub fn load_png_bytes(&mut self, data: &[u8]) -> Result<(), String> {
            let decoder = png::Decoder::new(data);
            let mut reader = decoder.read_info().map_err(|e| format!("Failed to read PNG info: {}", e))?;

            let mut buffer = vec![0u8; reader.output_buffer_size()];
            reader.next_frame(&mut buffer).map_err(|e| format!("Failed to decode PNG: {}", e))?;

            let width = reader.info().width;
            let height = reader.info().height;

            self.loaded_data = Some((buffer, width, height));
            Ok(())
        }

        pub fn data(&mut self) -> Option<(Vec<u8>, u32, u32)> {
            self.loaded_data.clone()
        }
    }
});
