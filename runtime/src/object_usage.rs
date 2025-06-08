use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use syn::visit::{self, Visit};
use syn::{Expr, File, ItemMacro, Local, Pat, Type};

struct UsageVisitor<'a> {
    object_names: &'a HashSet<String>,
    crate_name: &'a str,
    var_types: HashMap<String, String>,
    result: HashMap<String, HashSet<String>>,
}

impl<'a> UsageVisitor<'a> {
    fn new(object_names: &'a HashSet<String>, crate_name: &'a str) -> Self {
        Self { object_names, crate_name, var_types: HashMap::new(), result: HashMap::new() }
    }

    fn insert_call(&mut self, obj: &str, method: &str) {
        if obj == self.crate_name {
            return;
        }
        self.result.entry(obj.to_string()).or_default().insert(method.to_string());
    }
}

fn type_ident(ty: &Type) -> Option<String> {
    if let Type::Path(tp) = ty {
        if tp.qself.is_none() && tp.path.segments.len() == 1 {
            return Some(tp.path.segments[0].ident.to_string());
        }
    }
    None
}

fn path_first_last(path: &syn::Path) -> Option<(String, String)> {
    if let (Some(first), Some(last)) =
        (path.segments.first().map(|s| s.ident.to_string()), path.segments.last().map(|s| s.ident.to_string()))
    {
        Some((first, last))
    } else {
        None
    }
}

fn expr_object_call(expr: &Expr) -> Option<(String, String)> {
    if let Expr::Call(call) = expr {
        if let Expr::Path(p) = &*call.func {
            return path_first_last(&p.path);
        }
    }
    None
}

fn receiver_ident(expr: &Expr) -> Option<String> {
    if let Expr::Path(p) = expr {
        if p.qself.is_none() && p.path.segments.len() == 1 {
            return Some(p.path.segments[0].ident.to_string());
        }
    }
    None
}

impl<'ast, 'a> Visit<'ast> for UsageVisitor<'a> {
    fn visit_item_macro(&mut self, node: &'ast ItemMacro) {
        if node.mac.path.is_ident("object")
            || (node.mac.path.segments.len() == 2
                && node.mac.path.segments[0].ident == "hotline"
                && node.mac.path.segments[1].ident == "object")
        {
            if let Ok(file) = syn::parse2::<File>(node.mac.tokens.clone()) {
                self.visit_file(&file);
            }
        }
        visit::visit_item_macro(self, node);
    }

    fn visit_local(&mut self, node: &'ast Local) {
        // Extract identifier name from pattern like `let foo: Bar = ...` or `let foo = ...`
        if let Pat::Type(pat_type) = &node.pat {
            if let Pat::Ident(pat_ident) = &*pat_type.pat {
                let var = pat_ident.ident.to_string();
                if let Some(tname) = type_ident(&pat_type.ty) {
                    if self.object_names.contains(&tname) {
                        self.var_types.insert(var.clone(), tname);
                    }
                }
                if let Some(init) = &node.init {
                    if let Some((obj, method)) = expr_object_call(&init.expr) {
                        if self.object_names.contains(&obj) {
                            self.var_types.insert(var.clone(), obj.clone());
                            // treat constructor call as method usage
                            self.insert_call(&obj, &method);
                        }
                    }
                }
            }
        } else if let Pat::Ident(pat_ident) = &node.pat {
            let var = pat_ident.ident.to_string();
            if let Some(init) = &node.init {
                if let Some((obj, method)) = expr_object_call(&init.expr) {
                    if self.object_names.contains(&obj) {
                        self.var_types.insert(var.clone(), obj.clone());
                        self.insert_call(&obj, &method);
                    }
                }
            }
        }
        visit::visit_local(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if let Some(var) = receiver_ident(&node.receiver) {
            if let Some(obj) = self.var_types.get(&var) {
                let obj_name = obj.clone();
                self.insert_call(&obj_name, &node.method.to_string());
            }
        }
        visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast syn::ExprCall) {
        if let Expr::Path(p) = &*node.func {
            if let Some((obj, method)) = path_first_last(&p.path) {
                if self.object_names.contains(&obj) {
                    self.insert_call(&obj, &method);
                }
            }
        }
        visit::visit_expr_call(self, node);
    }
}

fn analyze_file(path: &Path, object_names: &HashSet<String>, crate_name: &str) -> HashMap<String, HashSet<String>> {
    let content = fs::read_to_string(path).expect("read lib.rs");
    let file = syn::parse_file(&content).expect("parse file");
    let mut visitor = UsageVisitor::new(object_names, crate_name);
    visitor.visit_file(&file);
    visitor.result
}

fn main() {
    let objects_dir = Path::new("objects");
    let mut object_names: HashSet<String> = HashSet::new();
    if let Ok(entries) = fs::read_dir(objects_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    object_names.insert(name.to_string());
                }
            }
        }
    }

    let mut results: Vec<(String, HashMap<String, HashSet<String>>)> = Vec::new();
    for obj in &object_names {
        let lib_path = objects_dir.join(obj).join("src").join("lib.rs");
        if lib_path.exists() {
            let usage = analyze_file(&lib_path, &object_names, obj);
            results.push((obj.clone(), usage));
        }
    }

    for (obj, usage) in results {
        println!("{}:", obj);
        for (ext_obj, methods) in usage {
            let mut methods: Vec<_> = methods.into_iter().collect();
            methods.sort();
            println!("  {} -> {}", ext_obj, methods.join(", "));
        }
    }
}
