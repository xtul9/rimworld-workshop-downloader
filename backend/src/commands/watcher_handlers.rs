// Mod watcher commands

use tauri::{command, AppHandle};
use crate::services::{get_mod_watcher, validate_mods_path};

/// Start watching the mods folder for changes
#[command]
pub async fn start_mod_watcher(
    app: AppHandle,
    mods_path: String,
) -> Result<(), String> {
    let path = validate_mods_path(&mods_path)?;
    
    let watcher = get_mod_watcher();
    let mut watcher_guard = watcher.lock().await;
    
    watcher_guard.start_watching(path, app).await
        .map_err(|e| format!("Failed to start mod watcher: {}", e))?;
    
    Ok(())
}

/// Stop watching the mods folder
#[command]
pub async fn stop_mod_watcher() -> Result<(), String> {
    let watcher = get_mod_watcher();
    let mut watcher_guard = watcher.lock().await;
    
    watcher_guard.stop_watching().await;
    
    Ok(())
}

