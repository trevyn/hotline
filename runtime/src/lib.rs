use hotline::{HotlineObject, LibraryRegistry, ObjectHandle};
use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashSet;
use std::path::Path;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub const RUSTC_COMMIT: &str = env!("RUSTC_COMMIT_HASH");

pub struct DirectRuntime {
    library_registry: LibraryRegistry,
    watcher_thread: Option<thread::JoinHandle<()>>,
    root_objects: Arc<Mutex<Vec<(String, ObjectHandle)>>>, // (type_name, handle)
}

impl DirectRuntime {
    pub fn new() -> Self {
        Self {
            library_registry: LibraryRegistry::new(),
            watcher_thread: None,
            root_objects: Arc::new(Mutex::new(Vec::new())),
        }
    }

    #[cfg(target_os = "macos")]
    pub fn new_with_custom_loader() -> Self {
        Self {
            library_registry: LibraryRegistry::new_with_custom_loader(),
            watcher_thread: None,
            root_objects: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn register(&mut self, obj: Box<dyn HotlineObject>) -> ObjectHandle {
        Arc::new(Mutex::new(obj))
    }

    pub fn register_root(&mut self, type_name: &str, handle: ObjectHandle) {
        if let Ok(mut roots) = self.root_objects.lock() {
            roots.push((type_name.to_string(), handle));
        }
    }

    pub fn hot_reload(&mut self, lib_path: &str, type_name: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.library_registry.load(lib_path)?;

        // Trigger migrations on root objects
        let reloaded_libs = {
            let mut set = HashSet::new();
            set.insert(type_name.to_string());
            set
        };

        self.trigger_migrations(&reloaded_libs)?;
        Ok(())
    }

    fn trigger_migrations(&self, reloaded_libs: &HashSet<String>) -> Result<(), Box<dyn std::error::Error>> {
        if let Ok(roots) = self.root_objects.lock() {
            for (_, handle) in roots.iter() {
                if let Ok(mut guard) = handle.lock() {
                    guard.migrate_children(reloaded_libs)?;
                }
            }
        }
        Ok(())
    }

    // Create object from loaded library
    pub fn create_from_lib(
        &mut self,
        lib_name: &str,
        type_name: &str,
    ) -> Result<ObjectHandle, Box<dyn std::error::Error>> {
        // Get a pointer to self that we can use as 'static
        // This is safe because we know the runtime is leaked in main.rs
        let self_ptr = self as *const DirectRuntime;
        let lib_registry = unsafe { &(*self_ptr).library_registry as &'static LibraryRegistry };

        // Set the library registry in thread-local storage before creating objects
        // This allows constructors to create other objects
        hotline::set_library_registry(lib_registry);

        // Create the object
        let mut obj = lib_registry.call_constructor(lib_name, type_name, RUSTC_COMMIT)?;

        // Store the registry on the object so it can create other objects later
        obj.set_registry(lib_registry);

        let handle = self.register(obj);
        Ok(handle)
    }

    pub fn library_registry(&self) -> &LibraryRegistry {
        &self.library_registry
    }

    pub fn start_watching(&mut self, path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = path.to_string();
        let registry = self.library_registry.clone();

        // Clone root_objects Arc for the watcher thread
        let root_objects = self.root_objects.clone();

        let handle = thread::spawn(move || match watch_and_reload_files(path, registry, root_objects) {
            Ok(()) => eprintln!("File watcher thread exited normally"),
            Err(e) => {
                eprintln!("File watcher error: {}", e);
                panic!("File watcher thread failed: {}", e);
            }
        });

        self.watcher_thread = Some(handle);
        Ok(())
    }
}

fn watch_and_reload_files(
    path: String,
    registry: LibraryRegistry,
    root_objects: Arc<Mutex<Vec<(String, ObjectHandle)>>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = channel();

    let mut watcher = RecommendedWatcher::new(tx, Config::default().with_poll_interval(Duration::from_millis(500)))?;

    watcher.watch(Path::new(&path), RecursiveMode::Recursive)?;

    // Track file hashes to detect actual changes
    let mut file_hashes = std::collections::HashMap::new();

    eprintln!("File watcher thread started successfully");

    loop {
        match rx.recv() {
            Ok(res) => match res {
                Ok(event) => {
                    use notify::EventKind;
                    match event.kind {
                        EventKind::Modify(_) | EventKind::Create(_) => {
                            for event_path in event.paths {
                                if event_path.extension().map_or(false, |ext| ext == "rs") {
                                    let now =
                                        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
                                    eprintln!(
                                        "[{}.{}] File event: {:?} for path: {:?}",
                                        now.as_secs() % 3600,
                                        now.subsec_millis(),
                                        event.kind,
                                        event_path
                                    );
                                    // Find which object this file belongs to
                                    if let Some(object_name) = find_object_for_file(&event_path) {
                                        // Check if file content actually changed
                                        match std::fs::read(&event_path) {
                                            Ok(contents) => {
                                                let hash = xxhash_rust::xxh3::xxh3_64(&contents);

                                                if let Some(&prev_hash) = file_hashes.get(&event_path) {
                                                    if prev_hash == hash {
                                                        eprintln!(
                                                            "File {} has same hash, skipping",
                                                            event_path.display()
                                                        );
                                                        continue; // No actual change
                                                    }
                                                }

                                                file_hashes.insert(event_path.clone(), hash);
                                                eprintln!("Detected change in {}, recompiling...", object_name);

                                                // Compile
                                                match std::process::Command::new("cargo")
                                                    .args(["build", "--release", "-p", &object_name])
                                                    .status()
                                                {
                                                    Ok(status) => {
                                                        if !status.success() {
                                                            eprintln!("Cargo build failed for {}", object_name);
                                                            continue;
                                                        }
                                                    }
                                                    Err(e) => {
                                                        eprintln!(
                                                            "Failed to run cargo build for {}: {}",
                                                            object_name, e
                                                        );
                                                        continue;
                                                    }
                                                }

                                                // Reload
                                                #[cfg(target_os = "macos")]
                                                let lib_path = format!("target/release/lib{}.dylib", object_name);
                                                #[cfg(target_os = "linux")]
                                                let lib_path = format!("target/release/lib{}.so", object_name);
                                                #[cfg(target_os = "windows")]
                                                let lib_path = format!("target/release/{}.dll", object_name);

                                                if let Err(e) = registry.load(&lib_path) {
                                                    eprintln!("Failed to reload {}: {}", object_name, e);
                                                } else {
                                                    eprintln!("Successfully reloaded {}", object_name);

                                                    // Trigger migrations synchronously
                                                    let mut reloaded_libs = HashSet::new();
                                                    reloaded_libs.insert(object_name.clone());

                                                    if let Ok(roots) = root_objects.lock() {
                                                        for (_, handle) in roots.iter() {
                                                            if let Ok(mut guard) = handle.lock() {
                                                                if let Err(e) = guard.migrate_children(&reloaded_libs) {
                                                                    eprintln!(
                                                                        "Migration failed for {}: {}",
                                                                        object_name, e
                                                                    );
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                eprintln!("Failed to read file {}: {}", event_path.display(), e);
                                                continue;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Err(e) => eprintln!("Watch error: {:?}", e),
            },
            Err(e) => {
                eprintln!("Channel receive error: {:?}", e);
                return Err(format!("Watcher channel closed: {:?}", e).into());
            }
        }
    }

    Ok(())
}

fn find_object_for_file(path: &Path) -> Option<String> {
    // Check if path is under objects/ directory
    let components: Vec<_> = path.components().collect();
    for (i, component) in components.iter().enumerate() {
        if component.as_os_str() == "objects" && i + 1 < components.len() {
            if let Some(object_name) = components[i + 1].as_os_str().to_str() {
                // Debug: log what we found
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
                eprintln!(
                    "[{}.{}] Found object {} for path {:?}",
                    now.as_secs() % 3600,
                    now.subsec_millis(),
                    object_name,
                    path
                );
                return Some(object_name.to_string());
            }
        }
    }
    None
}

pub fn compile_all_objects(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    // Collect all object crate names first
    let mut packages = Vec::new();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        if entry.path().is_dir() {
            if let Some(name) = entry.file_name().to_str() {
                packages.push(name.to_string());
            }
        }
    }

    if packages.is_empty() {
        return Ok(());
    }

    // Invoke cargo once with multiple -p arguments
    eprintln!("Compiling objects: {:?}", packages);
    let mut cmd = std::process::Command::new("cargo");
    cmd.arg("build").arg("--release");
    for pkg in &packages {
        cmd.arg("-p").arg(pkg);
    }

    let status = cmd.status()?;
    if !status.success() {
        return Err("cargo build failed".into());
    }

    Ok(())
}
