hotline::object!({
    #[derive(Clone, Default)]
    pub struct JSONLoader {
        loaded_data: Option<serde_json::Value>,
    }

    impl JSONLoader {
        pub fn load_json(&mut self, path: &str) -> Result<(), String> {
            let json_str = std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
            let value = serde_json::from_str(&json_str).map_err(|e| format!("Failed to parse JSON: {}", e))?;
            self.loaded_data = Some(value);
            Ok(())
        }

        pub fn parse_json(&mut self, json_str: &str) -> Result<(), String> {
            let value = serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {}", e))?;
            self.loaded_data = Some(value);
            Ok(())
        }

        // Parse into any object that has visitor methods (using Font as template)
        pub fn parse_into(&mut self, target: &mut Like<Font>) -> Result<(), String> {
            let data = self.loaded_data.as_ref().ok_or("no data loaded")?;

            // Call visit_start
            target.visit_start()?;

            // Parse top-level object
            if let Some(obj) = data.as_object() {
                for (key, value) in obj {
                    if let Some(simple) = self.value_to_string(value) {
                        target.visit_field(key, &simple)?;
                    } else if value.is_array() {
                        // Check if target wants to visit this array
                        if target.visit_array_start(key) {
                            for item in value.as_array().unwrap() {
                                target.visit_object_start()?;
                                if let Some(item_obj) = item.as_object() {
                                    for (k, v) in item_obj {
                                        if let Some(s) = self.value_to_string(v) {
                                            target.visit_object_field(k, &s)?;
                                        }
                                    }
                                }
                                target.visit_object_end()?;
                            }
                            target.visit_array_end(key)?;
                        }
                    }
                }
            }

            target.visit_end()?;

            Ok(())
        }

        fn value_to_string(&self, value: &serde_json::Value) -> Option<String> {
            match value {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                serde_json::Value::Bool(b) => Some(b.to_string()),
                _ => None,
            }
        }
    }
});
