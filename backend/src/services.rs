// Common services and utilities for commands

use std::path::{Path, PathBuf};
use crate::core::{SteamApi, Downloader, mod_watcher::ModWatcher};
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

// Shared instances for stateful services
static STEAM_API: OnceLock<Arc<Mutex<SteamApi>>> = OnceLock::new();
static DOWNLOADER: OnceLock<Arc<Mutex<Downloader>>> = OnceLock::new();
static MOD_WATCHER: OnceLock<Arc<Mutex<ModWatcher>>> = OnceLock::new();

/// Get or initialize the shared SteamApi instance
pub fn get_steam_api() -> Arc<Mutex<SteamApi>> {
    STEAM_API.get_or_init(|| {
        Arc::new(Mutex::new(SteamApi::new()))
    }).clone()
}

/// Get or initialize the shared Downloader instance
pub fn get_downloader() -> Arc<Mutex<Downloader>> {
    DOWNLOADER.get_or_init(|| {
        Arc::new(Mutex::new(Downloader::new(None)))
    }).clone()
}

/// Get or initialize the shared ModWatcher instance
pub fn get_mod_watcher() -> Arc<Mutex<ModWatcher>> {
    MOD_WATCHER.get_or_init(|| {
        Arc::new(Mutex::new(ModWatcher::new()))
    }).clone()
}

/// Validate that a path exists and is a directory
pub fn validate_mods_path(path: &str) -> Result<PathBuf, String> {
    let path_buf = PathBuf::from(path);
    
    if !path_buf.exists() {
        return Err(format!("Mods path does not exist: {}", path));
    }
    
    if !path_buf.is_dir() {
        return Err(format!("Mods path is not a directory: {}", path));
    }
    
    Ok(path_buf)
}

/// Extract folder name from mod path
pub fn extract_folder_name(mod_path: &Path) -> Result<String, String> {
    mod_path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid mod path".to_string())
}

/// Get mods path from a mod path (parent directory)
pub fn get_mods_path_from_mod_path(mod_path: &Path) -> Result<PathBuf, String> {
    mod_path
        .parent()
        .ok_or_else(|| "Cannot get mods path from mod path".to_string())
        .map(|p| p.to_path_buf())
}

/// Find all mod folders with the given mod ID
pub async fn find_all_mod_folders_with_id(mods_path: &Path, mod_id: &str) -> Result<Vec<PathBuf>, String> {
    use crate::core::mod_scanner::query_mod_id;
    
    let mut folders = Vec::new();
    let entries = std::fs::read_dir(mods_path)
        .map_err(|e| format!("Failed to read mods directory: {}", e))?;
    
    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();
        
        if path.is_dir() {
            if let Ok(Some(folder_mod_id)) = query_mod_id(&path) {
                if folder_mod_id == mod_id {
                    folders.push(path);
                }
            }
        }
    }
    
    Ok(folders)
}

/// Fetch time_updated for mods without details
pub async fn fetch_mod_times_updated(mod_ids: &[String]) -> std::collections::HashMap<String, i64> {
    use crate::core::mod_scanner::query_mod_batch;
    
    let mut mod_id_to_time_updated = std::collections::HashMap::new();
    
    if mod_ids.is_empty() {
        return mod_id_to_time_updated;
    }
    
    // Query in batches of 50 in parallel
    const BATCH_SIZE: usize = 50;
    let mut batch_futures = Vec::new();
    
    for batch_idx in 0..(mod_ids.len() + BATCH_SIZE - 1) / BATCH_SIZE {
        let start = batch_idx * BATCH_SIZE;
        let end = std::cmp::min(start + BATCH_SIZE, mod_ids.len());
        let batch: Vec<String> = mod_ids[start..end].iter().cloned().collect();
        let api_clone = get_steam_api();
        
        let future = async move {
            // Small delay to stagger requests
            if batch_idx > 0 {
                tokio::time::sleep(tokio::time::Duration::from_millis(100 * batch_idx as u64)).await;
            }
            
            // Use query_mod_batch for efficient batch querying
            match query_mod_batch(&batch, 0).await {
                Ok(details) => {
                    let mut result = std::collections::HashMap::new();
                    for detail in details {
                        result.insert(detail.publishedfileid, detail.time_updated);
                    }
                    Ok(result)
                }
                Err(_) => {
                    // If batch query fails, fall back to individual queries sequentially
                    let mut api = api_clone.lock().await;
                    let mut result = std::collections::HashMap::new();
                    
                    for mod_id in batch {
                        match api.get_file_details(&mod_id).await {
                            Ok(details) => {
                                result.insert(mod_id, details.time_updated);
                            }
                            Err(_) => {
                                // Fallback to current time
                                let current_time = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs() as i64;
                                result.insert(mod_id, current_time);
                            }
                        }
                    }
                    Ok(result)
                }
            }
        };
        batch_futures.push(future);
    }
    
    // Wait for all batches in parallel
    let batch_results: Vec<Result<std::collections::HashMap<String, i64>, Box<dyn std::error::Error + Send + Sync>>> = 
        futures::future::join_all(batch_futures).await;
    
    for result in batch_results {
        if let Ok(batch_map) = result {
            mod_id_to_time_updated.extend(batch_map);
        }
    }
    
    mod_id_to_time_updated
}

/// Write .ignoredupdate file for a mod folder
pub async fn write_ignore_update_file(folder_path: PathBuf, time_updated: i64) {
    let about_path = folder_path.join("About");
    let ignore_update_path = about_path.join(".ignoredupdate");
    let time_str = time_updated.to_string();
    
    tokio::task::spawn_blocking(move || {
        if let Err(e) = std::fs::create_dir_all(&about_path) {
            eprintln!("Failed to create About directory: {}", e);
            return;
        }
        if let Err(e) = std::fs::write(&ignore_update_path, time_str) {
            eprintln!("Failed to write .ignoredupdate file: {}", e);
        }
    }).await.ok();
}

/// Write .lastupdated file for a mod folder
pub async fn write_last_updated_file(folder_path: PathBuf, time_updated: i64) {
    let about_path = folder_path.join("About");
    let last_updated_path = about_path.join(".lastupdated");
    let time_str = time_updated.to_string();
    
    tokio::task::spawn_blocking(move || {
        if let Err(e) = std::fs::create_dir_all(&about_path) {
            eprintln!("Failed to create About directory: {}", e);
            return;
        }
        if let Err(e) = std::fs::write(&last_updated_path, time_str) {
            eprintln!("Failed to write .lastupdated file: {}", e);
        }
    }).await.ok();
}

