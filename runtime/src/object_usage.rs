use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use syn::visit::{self, Visit};
use syn::{
    Expr, File, Item, ItemImpl, ItemMacro, ItemStruct, Local, Pat, Type, braced,
    parse::{Parse, ParseStream},
};

struct UsageVisitor<'a> {
    object_names: &'a HashSet<String>,
    crate_name: &'a str,
    var_types: HashMap<String, String>,
    field_types: HashMap<String, String>,
    result: HashMap<String, HashSet<String>>,
}

struct ObjectInput {
    type_defs: Vec<Item>,
    struct_item: ItemStruct,
    impl_blocks: Vec<ItemImpl>,
}

impl Parse for ObjectInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let content;
        braced!(content in input);

        let mut type_defs = Vec::new();
        let mut struct_item = None;
        let mut impl_blocks = Vec::new();
        let mut found_main_struct = false;

        while !content.is_empty() {
            let item: Item = content.parse()?;
            match &item {
                Item::Struct(s) if !found_main_struct => {
                    let fork = content.fork();
                    if fork.peek(syn::Token![impl]) {
                        struct_item = Some(s.clone());
                        found_main_struct = true;
                    } else {
                        type_defs.push(item);
                    }
                }
                Item::Impl(i) if found_main_struct => {
                    impl_blocks.push(i.clone());
                }
                _ if !found_main_struct => {
                    type_defs.push(item);
                }
                _ => {
                    return Err(syn::Error::new_spanned(&item, "Unexpected item after impl blocks"));
                }
            }
        }

        let struct_item = struct_item
            .ok_or_else(|| syn::Error::new(content.span(), "Expected a main struct definition with impl blocks"))?;
        if impl_blocks.is_empty() {
            return Err(syn::Error::new(content.span(), "Expected at least one impl block after the struct"));
        }

        Ok(ObjectInput { type_defs, struct_item, impl_blocks })
    }
}

impl<'a> UsageVisitor<'a> {
    fn new(object_names: &'a HashSet<String>, crate_name: &'a str) -> Self {
        Self {
            object_names,
            crate_name,
            var_types: HashMap::new(),
            field_types: HashMap::new(),
            result: HashMap::new(),
        }
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

fn find_object_ident(ty: &Type, object_names: &HashSet<String>) -> Option<String> {
    match ty {
        Type::Path(tp) => {
            for segment in &tp.path.segments {
                if object_names.contains(&segment.ident.to_string()) {
                    return Some(segment.ident.to_string());
                }
                if let syn::PathArguments::AngleBracketed(ab) = &segment.arguments {
                    for arg in &ab.args {
                        if let syn::GenericArgument::Type(t) = arg {
                            if let Some(name) = find_object_ident(t, object_names) {
                                return Some(name);
                            }
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn extract_ident(pat: &Pat) -> Option<String> {
    match pat {
        Pat::Ident(id) => Some(id.ident.to_string()),
        Pat::Reference(r) => extract_ident(&r.pat),
        Pat::TupleStruct(ts) if ts.elems.len() == 1 => extract_ident(&ts.elems[0]),
        Pat::Tuple(t) if t.elems.len() == 1 => extract_ident(&t.elems[0]),
        Pat::Paren(p) => extract_ident(&p.pat),
        _ => None,
    }
}

fn field_expr_object(expr: &Expr, field_types: &HashMap<String, String>) -> Option<String> {
    if let Expr::Field(f) = expr {
        if let Expr::Path(base) = &*f.base {
            if base.path.is_ident("self") {
                if let syn::Member::Named(ident) = &f.member {
                    if let Some(t) = field_types.get(&ident.to_string()) {
                        return Some(t.clone());
                    }
                }
            }
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
            if let Ok(obj) = syn::parse2::<ObjectInput>(node.mac.tokens.clone()) {
                for item in &obj.type_defs {
                    self.visit_item(item);
                }
                self.visit_item_struct(&obj.struct_item);
                for imp in &obj.impl_blocks {
                    self.visit_item_impl(imp);
                }
            }
        }
        visit::visit_item_macro(self, node);
    }

    fn visit_item_struct(&mut self, node: &'ast syn::ItemStruct) {
        if node.ident == self.crate_name {
            for field in &node.fields {
                if let Some(ident) = &field.ident {
                    if let Some(obj) = find_object_ident(&field.ty, self.object_names) {
                        self.field_types.insert(ident.to_string(), obj);
                    }
                }
            }
        }
        visit::visit_item_struct(self, node);
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

    fn visit_expr_let(&mut self, node: &'ast syn::ExprLet) {
        if let Some(var) = extract_ident(&node.pat) {
            if let Some(obj) = expr_object_call(&node.expr)
                .and_then(|(o, _)| if self.object_names.contains(&o) { Some(o) } else { None })
                .or_else(|| field_expr_object(&node.expr, &self.field_types))
            {
                self.var_types.insert(var.clone(), obj);
            }
        }
        visit::visit_expr_let(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if let Some(var) = receiver_ident(&node.receiver) {
            if let Some(obj) = self.var_types.get(&var) {
                let name = obj.clone();
                self.insert_call(&name, &node.method.to_string());
            }
        } else if let Some(obj) = field_expr_object(&node.receiver, &self.field_types) {
            self.insert_call(&obj, &node.method.to_string());
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
        if let Some(obj) = field_expr_object(&node.func, &self.field_types) {
            self.insert_call(&obj, "call");
        }
        visit::visit_expr_call(self, node);
    }
}

fn analyze_file(path: &Path, object_names: &HashSet<String>, crate_name: &str) -> HashMap<String, HashSet<String>> {
    let content = fs::read_to_string(path).expect("read lib.rs");
    let file = syn::parse_file(&content).expect("parse file");
    let mut visitor = UsageVisitor::new(object_names, crate_name);
    visitor.visit_file(&file);
    // debug output: eprintln!("fields of {}: {:?}", crate_name, visitor.field_types);
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
