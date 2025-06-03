use runtime::shim_gen::generate_shims_from_dylib;
use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <path-to-dylib>", args[0]);
        std::process::exit(1);
    }

    let dylib_path = Path::new(&args[1]);

    match generate_shims_from_dylib(dylib_path) {
        Ok(shim_code) => {
            println!("{}", shim_code);
        }
        Err(e) => {
            eprintln!("Error generating shims: {}", e);
            std::process::exit(1);
        }
    }
}
