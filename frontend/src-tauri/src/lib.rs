use std::process::{Command, Child};
use std::sync::Mutex;
use tauri::Manager;

pub mod backend;
pub mod commands;

static NODE_PROCESS: Mutex<Option<Child>> = Mutex::new(None);

fn start_node_backend(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let child = if cfg!(debug_assertions) {
        // Development mode: use project backend directory with npm
        let current_dir = std::env::current_dir()?;
        let backend_path = current_dir
            .parent()
            .and_then(|p| p.parent())
            .ok_or("Cannot find project root directory")?
            .join("backend");
        
        if !backend_path.exists() {
            return Err(format!("Backend directory not found at: {:?}", backend_path).into());
        }

        Command::new("npm")
            .arg("run")
            .arg("dev")
            .current_dir(&backend_path)
            .spawn()?
    } else {
        // Production mode: use externalBin binary
        // In Tauri v2, externalBin binaries are available through the app handle
        let resource_dir = app
            .path()
            .resource_dir()
            .map_err(|e| format!("Cannot find resource directory: {}", e))?;
        
        // Determine which binary to use based on platform
        // Tauri automatically adds target triple suffix to externalBin names
        // So we need to construct the full name with target triple
        let binary_name = "rimworld-workshop-downloader-backend";
        
        // Get target triple (Tauri sets this during build)
        let target_triple = if cfg!(target_os = "linux") {
            "x86_64-unknown-linux-gnu"
        } else if cfg!(target_os = "windows") {
            "x86_64-pc-windows-msvc"
        } else if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                "aarch64-apple-darwin"
            } else {
                "x86_64-apple-darwin"
            }
        } else {
            return Err("Unsupported platform".into());
        };
        
        let binary_name_with_suffix = if cfg!(target_os = "windows") {
            format!("{}-{}.exe", binary_name, target_triple)
        } else {
            format!("{}-{}", binary_name, target_triple)
        };
        
        // Binary name without suffix (Tauri sometimes places binaries without target triple suffix)
        let binary_name_without_suffix = if cfg!(target_os = "windows") {
            format!("{}.exe", binary_name)
        } else {
            binary_name.to_string()
        };
        
        // Try multiple possible locations for externalBin binaries
        // Tauri places them in different locations depending on the package format
        // Note: In RPM/Deb packages, Tauri may place binaries in /usr/bin/ without target triple suffix
        let exe = std::env::current_exe()?;
        let exe_dir = exe.parent().ok_or("Cannot find application directory")?;
        
        let possible_paths = vec![
            // 1. /usr/bin/ without suffix (common for RPM/Deb packages)
            std::path::PathBuf::from("/usr/bin").join(&binary_name_without_suffix),
            // 2. Next to executable without suffix
            exe_dir.join(&binary_name_without_suffix),
            // 3. Resource directory with suffix (primary location for some formats)
            resource_dir.join(&binary_name_with_suffix),
            // 4. Resource directory without suffix
            resource_dir.join(&binary_name_without_suffix),
            // 5. Next to executable with suffix
            exe_dir.join(&binary_name_with_suffix),
            // 6. In lib directory (common for RPM/Deb packages)
            exe_dir.join("..").join("lib").join(&binary_name_with_suffix),
            exe_dir.join("..").join("lib64").join(&binary_name_with_suffix),
            exe_dir.join("..").join("lib").join(&binary_name_without_suffix),
            exe_dir.join("..").join("lib64").join(&binary_name_without_suffix),
            // 7. In share directory
            exe_dir.join("..").join("share").join("rimworld-workshop-downloader").join(&binary_name_with_suffix),
            exe_dir.join("..").join("share").join("rimworld-workshop-downloader").join(&binary_name_without_suffix),
            // 8. Standard Linux library paths
            std::path::PathBuf::from("/usr/lib").join(&binary_name_with_suffix),
            std::path::PathBuf::from("/usr/lib64").join(&binary_name_with_suffix),
            std::path::PathBuf::from("/usr/local/lib").join(&binary_name_with_suffix),
            std::path::PathBuf::from("/usr/lib").join(&binary_name_without_suffix),
            std::path::PathBuf::from("/usr/lib64").join(&binary_name_without_suffix),
            // 9. App-specific lib directory
            std::path::PathBuf::from("/usr/lib/rimworld-workshop-downloader").join(&binary_name_with_suffix),
            std::path::PathBuf::from("/usr/share/rimworld-workshop-downloader").join(&binary_name_with_suffix),
            std::path::PathBuf::from("/usr/lib/rimworld-workshop-downloader").join(&binary_name_without_suffix),
            std::path::PathBuf::from("/usr/share/rimworld-workshop-downloader").join(&binary_name_without_suffix),
        ];
        
        let mut binary_path = None;
        for path in &possible_paths {
            if path.exists() {
                binary_path = Some(path.clone());
                eprintln!("Found backend binary at: {:?}", path);
                break;
            }
        }
        
        let binary_path = binary_path.ok_or_else(|| {
            let tried = possible_paths.iter()
                .map(|p| p.to_string_lossy().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("External binary not found. Tried: {}", tried)
        })?;

        // Check if binary is executable (but don't try to change permissions in production)
        // The binary should already have correct permissions from the package installation
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = std::fs::metadata(&binary_path) {
                let perms = metadata.permissions();
                let is_executable = perms.mode() & 0o111 != 0;
                eprintln!("Binary permissions: {:o}, executable: {}", perms.mode(), is_executable);
                if !is_executable {
                    eprintln!("Warning: Binary is not executable, but we won't change permissions in production");
                }
            }
        }

        eprintln!("Attempting to spawn backend process: {:?}", binary_path);
        
        // Convert PathBuf to string for better error messages
        let binary_path_str = binary_path.to_string_lossy().to_string();
        
        // Try to spawn the process
        // Note: We don't redirect stdout/stderr to allow backend to log properly
        let mut child = match Command::new(&binary_path_str)
            .spawn()
        {
            Ok(child) => {
                eprintln!("Backend process spawned successfully (PID: {:?})", child.id());
                child
            }
            Err(e) => {
                eprintln!("Failed to spawn backend process: {}", e);
                eprintln!("Binary path: {}", binary_path_str);
                eprintln!("Error kind: {:?}", e.kind());
                eprintln!("Error details: {:?}", e);
                
                // Try to get more information about the error
                if let std::io::ErrorKind::PermissionDenied = e.kind() {
                    eprintln!("Permission denied - checking file permissions...");
                    if let Ok(metadata) = std::fs::metadata(&binary_path) {
                        eprintln!("File metadata: {:?}", metadata);
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            eprintln!("File permissions: {:o}", metadata.permissions().mode());
                        }
                    }
                }
                
                return Err(format!("Failed to spawn backend process: {} (path: {})", e, binary_path_str).into());
            }
        };
        
        // Wait a moment and check if the process is still running
        std::thread::sleep(std::time::Duration::from_millis(100));
        match child.try_wait() {
            Ok(Some(status)) => {
                eprintln!("Warning: Backend process exited immediately with status: {:?}", status);
                return Err(format!("Backend process exited immediately with status: {:?}", status).into());
            }
            Ok(None) => {
                eprintln!("Backend process is running (PID: {:?})", child.id());
            }
            Err(e) => {
                eprintln!("Warning: Could not check process status: {}", e);
            }
        }
        
        child
    };

    if let Ok(mut process) = NODE_PROCESS.lock() {
        *process = Some(child);
    }

    Ok(())
}

fn stop_node_backend() {
    if let Ok(mut process) = NODE_PROCESS.lock() {
        if let Some(mut child) = process.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[tauri::command]
fn open_devtools(app: tauri::AppHandle) {
    // Open devtools for the main window
    // Requires "devtools" feature to be enabled in Cargo.toml
    if let Some(window) = app.get_webview_window("main") {
        window.open_devtools();
    }
}



#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Fix Wayland protocol error (Error 71)
    // See: https://github.com/tauri-apps/tauri/issues/10702
    if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    
    // Set GDK_BACKEND for Wayland if not already set
    if std::env::var("GDK_BACKEND").unwrap_or_default().is_empty() 
        && std::env::var("XDG_SESSION_TYPE").unwrap_or_default() == "wayland" {
        std::env::set_var("GDK_BACKEND", "wayland");
    }
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_devtools::init())
        .invoke_handler(tauri::generate_handler![
            open_devtools,
            commands::query_mods,
            commands::update_mods,
            commands::check_backup,
            commands::check_backups,
            commands::restore_backup,
            commands::restore_backups,
            commands::ignore_update,
            commands::get_file_details,
            commands::get_file_details_batch,
            commands::is_collection,
            commands::is_collection_batch,
            commands::get_collection_details,
            commands::download_mod,
        ])
        .setup(|app| {
            // Check if backend is already running (e.g., started by run-dev.sh)
            // Only start backend if not already running
            let backend_running = std::net::TcpStream::connect("127.0.0.1:5000").is_ok();
            
            if !backend_running {
                if let Err(e) = start_node_backend(app.handle()) {
                    eprintln!("Warning: Failed to start Node.js backend: {}", e);
                    eprintln!("You may need to start it manually: cd backend && npm run dev");
                }
            }

            if let Some(window) = app.get_webview_window("main") {
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        stop_node_backend();
                    }
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    // Ensure backend is stopped when Tauri exits
    stop_node_backend();
}
