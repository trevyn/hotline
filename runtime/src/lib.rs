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

        // Create a channel for reload notifications
        let (reload_tx, reload_rx) = std::sync::mpsc::channel::<String>();

        // Clone root_objects Arc for the migration thread
        let root_objects = self.root_objects.clone();

        let handle = thread::spawn(move || {
            if let Err(e) = watch_and_reload_files(path, registry, reload_tx) {
                eprintln!("File watcher error: {}", e);
            }
        });

        // Spawn another thread to handle reload notifications
        thread::spawn(move || {
            while let Ok(type_name) = reload_rx.recv() {
                let mut reloaded_libs = HashSet::new();
                reloaded_libs.insert(type_name.clone());

                // Trigger migrations on all root objects
                if let Ok(roots) = root_objects.lock() {
                    for (_, handle) in roots.iter() {
                        if let Ok(mut guard) = handle.lock() {
                            if let Err(e) = guard.migrate_children(&reloaded_libs) {
                                eprintln!("Migration failed for {}: {}", type_name, e);
                            }
                        }
                    }
                }
            }
        });

        self.watcher_thread = Some(handle);
        Ok(())
    }
}

fn watch_and_reload_files(
    path: String,
    registry: LibraryRegistry,
    reload_tx: std::sync::mpsc::Sender<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (tx, rx) = channel();

    let mut watcher = RecommendedWatcher::new(tx, Config::default().with_poll_interval(Duration::from_millis(500)))?;

    watcher.watch(Path::new(&path), RecursiveMode::Recursive)?;

    // Track file hashes to detect actual changes
    let mut file_hashes = std::collections::HashMap::new();

    for res in rx {
        match res {
            Ok(event) => {
                use notify::EventKind;
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        for event_path in event.paths {
                            if event_path.extension().map_or(false, |ext| ext == "rs") {
                                // Find which object this file belongs to
                                if let Some(object_name) = find_object_for_file(&event_path) {
                                    // Check if file content actually changed
                                    if let Ok(contents) = std::fs::read(&event_path) {
                                        let hash = xxhash_rust::xxh3::xxh3_64(&contents);

                                        if let Some(&prev_hash) = file_hashes.get(&event_path) {
                                            if prev_hash == hash {
                                                continue; // No actual change
                                            }
                                        }

                                        file_hashes.insert(event_path.clone(), hash);

                                        eprintln!("Detected change in {}, recompiling...", object_name);

                                        // Compile
                                        let status = std::process::Command::new("cargo")
                                            .args(["build", "--release", "-p", &object_name])
                                            .status()?;

                                        if !status.success() {
                                            eprintln!("Cargo build failed for {}", object_name);
                                            continue;
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
                                            // Notify about successful reload so migrations can be triggered
                                            let _ = reload_tx.send(object_name.clone());
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
