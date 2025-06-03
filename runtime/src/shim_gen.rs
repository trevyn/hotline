use std::collections::HashMap;
use std::path::Path;

/// Represents a parsed method signature
#[derive(Debug)]
struct MethodSignature {
    type_name: String,
    method_name: String,
    params: Vec<(String, String)>, // (param_name, param_type)
    return_type: String,
}

/// Parse encoded symbol names back into method signatures
fn parse_symbol(symbol: &str) -> Option<MethodSignature> {
    // Format: TypeName__method_name____param1_type1__param2_type2__to__return_type
    let parts: Vec<&str> = symbol.split("__").collect();

    if parts.len() < 3 {
        return None;
    }

    let type_name = parts[0].to_string();
    let method_name = parts[1].to_string();

    // Find the "to" separator
    let to_idx = parts.iter().position(|&p| p == "to")?;

    // Parse parameters (between method name and "to")
    let mut params = Vec::new();
    let param_parts = &parts[2..to_idx];

    // Skip the first part if it's empty (from ____)
    let param_start = if param_parts.first() == Some(&"") { 1 } else { 0 };

    // The format is: param_name_type or just type for anonymous params
    for part in &param_parts[param_start..] {
        if part.contains('_') {
            // For complex types like "buffer_ref_mut_slice_u8_endslice",
            // we need smarter parsing
            if part.contains("_ref_mut_slice_") || part.contains("_ref_slice_") {
                // Find where the type starts
                if let Some(type_start) = part.find("_ref_") {
                    let param_name = part[..type_start].to_string();
                    let param_type = part[type_start + 1..].to_string();
                    params.push((param_name, param_type));
                } else {
                    params.push(("".to_string(), part.to_string()));
                }
            } else {
                // Simple types - split by last underscore
                let underscore_pos = part.rfind('_').unwrap();
                let param_name = part[..underscore_pos].to_string();
                let param_type = part[underscore_pos + 1..].to_string();
                params.push((param_name, param_type));
            }
        } else {
            // Just a type, no name
            params.push(("".to_string(), part.to_string()));
        }
    }

    // Get return type (after "to")
    let return_type =
        if to_idx + 1 < parts.len() { parts[to_idx + 1..].join("__") } else { "unit".to_string() };

    Some(MethodSignature { type_name, method_name, params, return_type })
}

/// Read symbols from a dylib file
fn read_symbols(dylib_path: &Path) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    use std::process::Command;

    // Use nm to list symbols
    let output = Command::new("nm")
        .arg("-gU") // global symbols, undefined suppressed
        .arg(dylib_path)
        .output()?;

    let stdout = String::from_utf8(output.stdout)?;

    // Parse nm output - format is "address type symbol_name"
    let symbols: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3 && parts[1] == "T" {
                let symbol = parts[2..].join(" ");
                // Strip leading underscore on macOS
                if symbol.starts_with('_') { Some(symbol[1..].to_string()) } else { Some(symbol) }
            } else {
                None
            }
        })
        .collect();

    Ok(symbols)
}

/// Generate Rust shim code from parsed signatures
fn generate_shim(type_name: &str, methods: Vec<MethodSignature>) -> String {
    let mut code = String::new();

    // Generate struct definition
    code.push_str(&format!("/// Auto-generated shim for {}\n", type_name));
    code.push_str(&format!("pub struct {} {{\n", type_name));
    code.push_str("    runtime: std::sync::Arc<std::sync::Mutex<DirectRuntime>>,\n");
    code.push_str("    handle: ObjectHandle,\n");
    code.push_str("}\n\n");

    // Generate impl block
    code.push_str(&format!("impl {} {{\n", type_name));

    // Constructor
    code.push_str("    pub fn new(runtime: std::sync::Arc<std::sync::Mutex<DirectRuntime>>, handle: ObjectHandle) -> Self {\n");
    code.push_str("        Self { runtime, handle }\n");
    code.push_str("    }\n\n");

    // Generate methods
    for method in methods {
        if method.method_name == "new" {
            continue; // Skip constructor
        }

        // Determine if it's a getter, setter, or regular method
        if method.method_name.starts_with("get_") && method.params.len() == 1 {
            // Getter
            let return_type = convert_type(&method.return_type);

            code.push_str(&format!(
                "    pub fn {}(&self) -> Result<{}, Box<dyn std::error::Error>> {{\n",
                method.method_name, return_type
            ));
            code.push_str(&format!(
                "        self.runtime.lock().unwrap().call_getter::<{}>(\n",
                return_type
            ));
            code.push_str(&format!("            self.handle,\n"));
            code.push_str(&format!("            \"{}\",\n", type_name));
            code.push_str(&format!("            \"lib{}\",\n", type_name.to_lowercase()));
            code.push_str(&format!("            \"{}\"\n", method.method_name));
            code.push_str("        )\n");
            code.push_str("    }\n\n");
        } else if method.method_name.starts_with("set_") && method.params.len() == 2 {
            // Setter
            let value_type = convert_type(&method.params[1].1);

            code.push_str(&format!(
                "    pub fn {}(&self, value: {}) -> Result<(), Box<dyn std::error::Error>> {{\n",
                method.method_name, value_type
            ));
            code.push_str("        self.runtime.lock().unwrap().call_setter(\n");
            code.push_str(&format!("            self.handle,\n"));
            code.push_str(&format!("            \"{}\",\n", type_name));
            code.push_str(&format!("            \"lib{}\",\n", type_name.to_lowercase()));
            code.push_str(&format!("            \"{}\",\n", method.method_name));
            code.push_str("            value\n");
            code.push_str("        )\n");
            code.push_str("    }\n\n");
        } else {
            // Regular method
            let mut param_list = Vec::new();
            let mut arg_list = Vec::new();

            // Skip the first param (obj)
            for (param_name, param_type) in method.params.iter().skip(1) {
                let rust_type = convert_type(param_type);
                param_list.push(format!("{}: {}", param_name, rust_type));
                arg_list.push(format!("Box::new({})", param_name));
            }

            let params_str = param_list.join(", ");
            let args_str = arg_list.join(", ");

            code.push_str(&format!(
                "    pub fn {}(&self, {}) -> Result<(), Box<dyn std::error::Error>> {{\n",
                method.method_name, params_str
            ));
            code.push_str(&format!(
                "        let args: Vec<Box<dyn std::any::Any>> = vec![{}];\n",
                args_str
            ));
            code.push_str("        self.runtime.lock().unwrap().call_method(\n");
            code.push_str(&format!("            self.handle,\n"));
            code.push_str(&format!("            \"{}\",\n", type_name));
            code.push_str(&format!("            \"lib{}\",\n", type_name.to_lowercase()));
            code.push_str(&format!("            \"{}\",\n", method.method_name));
            code.push_str("            args\n");
            code.push_str("        )?;\n");
            code.push_str("        Ok(())\n");
            code.push_str("    }\n\n");
        }
    }

    code.push_str("}\n");

    code
}

/// Convert encoded type names to Rust types
fn convert_type(encoded: &str) -> String {
    match encoded {
        "f64" => "f64".to_string(),
        "i64" => "i64".to_string(),
        "i32" => "i32".to_string(),
        "bool" => "bool".to_string(),
        "unit" => "()".to_string(),
        "Box_lt_dyn_Any_gt" => "Box<dyn std::any::Any>".to_string(),
        s if s.starts_with("ref_mut_slice_") && s.ends_with("_endslice") => {
            let inner = &s[14..s.len() - 9]; // Extract type between ref_mut_slice_ and _endslice
            format!("&mut [{}]", inner)
        }
        s if s.starts_with("ref_slice_") && s.ends_with("_endslice") => {
            let inner = &s[10..s.len() - 9]; // Extract type between ref_slice_ and _endslice
            format!("&[{}]", inner)
        }
        _ => encoded.to_string(),
    }
}

/// Main function to generate shims from a dylib
pub fn generate_shims_from_dylib(dylib_path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let symbols = read_symbols(dylib_path)?;

    // Group methods by type
    let mut type_methods: HashMap<String, Vec<MethodSignature>> = HashMap::new();

    for symbol in symbols {
        if let Some(sig) = parse_symbol(&symbol) {
            type_methods.entry(sig.type_name.clone()).or_insert_with(Vec::new).push(sig);
        }
    }

    // Generate shims for each type
    let mut all_code = String::new();
    all_code.push_str("// Auto-generated shims\n");
    all_code.push_str("use crate::{DirectRuntime, ObjectHandle};\n");
    all_code.push_str("use std::sync::{Arc, Mutex};\n\n");

    for (type_name, methods) in type_methods {
        all_code.push_str(&generate_shim(&type_name, methods));
        all_code.push('\n');
    }

    Ok(all_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_symbol() {
        let symbol = "Rect__get_x____obj_ref_dyn_Any__to__f64";
        let parsed = parse_symbol(symbol).unwrap();

        assert_eq!(parsed.type_name, "Rect");
        assert_eq!(parsed.method_name, "get_x");
        assert_eq!(parsed.params.len(), 1);
        assert_eq!(parsed.params[0], ("obj_ref_dyn".to_string(), "Any".to_string()));
        assert_eq!(parsed.return_type, "f64");
    }

    #[test]
    fn test_parse_setter_symbol() {
        let symbol = "Rect__set_x____obj_mut_dyn_Any__x_f64__to__unit";
        let parsed = parse_symbol(symbol).unwrap();

        assert_eq!(parsed.type_name, "Rect");
        assert_eq!(parsed.method_name, "set_x");
        assert_eq!(parsed.params.len(), 2);
        assert_eq!(parsed.params[0], ("obj_mut_dyn".to_string(), "Any".to_string()));
        assert_eq!(parsed.params[1], ("x".to_string(), "f64".to_string()));
        assert_eq!(parsed.return_type, "unit");
    }

    #[test]
    fn test_parse_method_symbol() {
        let symbol = "Rect__move_by____obj_mut_dyn_Any__dx_f64__dy_f64__to__unit";
        let parsed = parse_symbol(symbol).unwrap();

        assert_eq!(parsed.type_name, "Rect");
        assert_eq!(parsed.method_name, "move_by");
        assert_eq!(parsed.params.len(), 3);
        assert_eq!(parsed.params[0], ("obj_mut_dyn".to_string(), "Any".to_string()));
        assert_eq!(parsed.params[1], ("dx".to_string(), "f64".to_string()));
        assert_eq!(parsed.params[2], ("dy".to_string(), "f64".to_string()));
        assert_eq!(parsed.return_type, "unit");
    }
}
