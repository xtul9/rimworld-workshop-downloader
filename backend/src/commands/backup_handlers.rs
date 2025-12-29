// Backup-related commands

use std::path::PathBuf;
use serde_json;
use tauri::{command, AppHandle};
use crate::services::{extract_folder_name, get_mods_path_from_mod_path};
use crate::core::access_check::ensure_directory_access;

/// Check if backup exists for a mod (optimized with spawn_blocking)
#[command]
pub async fn check_backup(
    mod_path: String,
    backup_directory: Option<String>,
) -> Result<serde_json::Value, String> {
    if let Some(backup_dir) = backup_directory {
        let mod_path_buf = PathBuf::from(&mod_path);
        let folder_name = extract_folder_name(&mod_path_buf)?;
        
        let backup_path = PathBuf::from(&backup_dir).join(&folder_name);
        
        // Use spawn_blocking for I/O operations to avoid blocking the async runtime
        let result = tokio::task::spawn_blocking(move || {
            if backup_path.exists() {
                match std::fs::metadata(&backup_path) {
                    Ok(metadata) => {
                        let backup_date = metadata.modified()
                            .or_else(|_| metadata.accessed())
                            .unwrap_or(std::time::SystemTime::now());
                        
                        Ok(serde_json::json!({
                            "hasBackup": true,
                            "backupPath": backup_path.to_string_lossy(),
                            "backupDate": backup_date.duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                        }))
                    }
                    Err(e) => Err(format!("Failed to get backup metadata: {}", e))
                }
            } else {
                Ok(serde_json::json!({
                    "hasBackup": false,
                    "backupPath": backup_path.to_string_lossy()
                }))
            }
        }).await
        .map_err(|e| format!("Task panicked: {:?}", e))??;
        
        Ok(result)
    } else {
        Ok(serde_json::json!({
            "hasBackup": false,
            "backupPath": null
        }))
    }
}

/// Check if backups exist for multiple mods (optimized batch version)
#[command]
pub async fn check_backups(
    mod_paths: Vec<String>,
    backup_directory: Option<String>,
) -> Result<serde_json::Value, String> {
    if mod_paths.is_empty() {
        return Ok(serde_json::json!({}));
    }
    
    if let Some(backup_dir) = backup_directory {
        let backup_dir_buf = PathBuf::from(&backup_dir);
        
        // Check all backups in parallel
        let mut check_futures = Vec::new();
        
        for mod_path in mod_paths {
            let mod_path_buf = PathBuf::from(&mod_path);
            let folder_name = match extract_folder_name(&mod_path_buf) {
                Ok(name) => name,
                Err(_) => continue, // Skip invalid paths
            };
            
            let backup_path = backup_dir_buf.join(&folder_name);
            let mod_path_clone = mod_path.clone();
            
            // Spawn blocking task for each backup check
            let future = tokio::task::spawn_blocking(move || {
                if backup_path.exists() {
                    match std::fs::metadata(&backup_path) {
                        Ok(metadata) => {
                            let backup_date = metadata.modified()
                                .or_else(|_| metadata.accessed())
                                .unwrap_or(std::time::SystemTime::now());
                            
                            Some(serde_json::json!({
                                "hasBackup": true,
                                "backupPath": backup_path.to_string_lossy(),
                                "backupDate": backup_date.duration_since(std::time::UNIX_EPOCH)
                                    .unwrap()
                                    .as_secs()
                            }))
                        }
                        Err(_) => Some(serde_json::json!({
                            "hasBackup": false,
                            "backupPath": backup_path.to_string_lossy()
                        }))
                    }
                } else {
                    Some(serde_json::json!({
                        "hasBackup": false,
                        "backupPath": backup_path.to_string_lossy()
                    }))
                }
            });
            
            check_futures.push((mod_path_clone, future));
        }
        
        // Wait for all checks to complete and build result map
        let mut results = serde_json::Map::new();
        for (mod_path, future) in check_futures {
            match future.await {
                Ok(Some(result)) => {
                    results.insert(mod_path, result);
                }
                Ok(None) => {
                    // Invalid path, skip
                }
                Err(_) => {
                    // Task panicked, skip
                }
            }
        }
        
        Ok(serde_json::Value::Object(results))
    } else {
        // No backup directory, return all false
        let mut results = serde_json::Map::new();
        for mod_path in mod_paths {
            results.insert(mod_path, serde_json::json!({
                "hasBackup": false,
                "backupPath": null
            }));
        }
        Ok(serde_json::Value::Object(results))
    }
}

/// Restore mod from backup (optimized with async I/O)
#[command]
pub async fn restore_backup(
    app: AppHandle,
    mod_path: String,
    backup_directory: String,
) -> Result<serde_json::Value, String> {
    
    let normalized_mod_path = PathBuf::from(&mod_path);
    let normalized_backup_directory = PathBuf::from(&backup_directory);
    
    // Check directory access to parent mods directory (write access is required for restore)
    // Use parent directory because the mod folder may not exist when restoring a deleted mod
    let mods_path = get_mods_path_from_mod_path(&normalized_mod_path)?;
    let mods_path_str = mods_path.to_string_lossy().to_string();
    ensure_directory_access(&app, &mods_path, &mods_path_str)?;
    
    // Safety check: ensure backupDirectory is not inside modPath (or vice versa)
    if normalized_mod_path.starts_with(&normalized_backup_directory) ||
       normalized_backup_directory.starts_with(&normalized_mod_path) {
        return Err("Backup directory cannot be inside mods path or vice versa. They must be separate directories.".to_string());
    }
    
    // Extract folder name from modPath
    let folder_name = extract_folder_name(&normalized_mod_path)?;
    
    let backup_path = normalized_backup_directory.join(folder_name);
    
    // Additional safety check
    if !backup_path.starts_with(&normalized_backup_directory) {
        return Err("Invalid backup path detected".to_string());
    }
    
    // Critical safety check: ensure backupPath and modPath are not the same
    if backup_path == normalized_mod_path {
        return Err("Backup path and mod path cannot be the same. Please ensure backup directory is different from mods directory.".to_string());
    }
    
    // Check if backup exists (async)
    let backup_exists = tokio::task::spawn_blocking({
        let backup_path = backup_path.clone();
        move || backup_path.exists()
    }).await
    .map_err(|e| format!("Task panicked: {:?}", e))?;
    
    if !backup_exists {
        return Err("Backup not found".to_string());
    }
    
    // Ignore this path in mod watcher during restore operation
    use crate::services::{ignore_path_in_watcher, WatcherIgnoreGuard};
    ignore_path_in_watcher(normalized_mod_path.clone()).await;
    let _guard = WatcherIgnoreGuard::new(normalized_mod_path.clone()).await;
    
    // Remove current mod folder (async)
    let mod_path_clone = normalized_mod_path.clone();
    tokio::task::spawn_blocking(move || {
        if mod_path_clone.exists() {
            std::fs::remove_dir_all(&mod_path_clone)
                .map_err(|e| format!("Failed to remove current mod folder: {}", e))?;
        }
        Ok::<(), String>(())
    }).await
    .map_err(|e| format!("Task panicked: {:?}", e))??;
    
    // Copy backup to mods folder (async)
    use crate::core::mod_manager::copy_dir_all_async;
    let backup_path_clone = backup_path.clone();
    let mod_path_clone2 = normalized_mod_path.clone();
    copy_dir_all_async(&backup_path_clone, &mod_path_clone2).await
        .map_err(|e| format!("Failed to copy backup: {}", e))?;
    
    // Delete the backup (async) - only after successful copy
    let backup_path_clone2 = backup_path.clone();
    tokio::task::spawn_blocking(move || {
        std::fs::remove_dir_all(&backup_path_clone2)
            .map_err(|e| format!("Failed to delete backup: {}", e))
    }).await
    .map_err(|e| format!("Task panicked: {:?}", e))??;
    
    // Manually unignore the path (this consumes the guard and prevents Drop from running)
    // If we reach here, the operation was successful
    _guard.unignore().await;
    
    Ok(serde_json::json!({
        "message": "Backup restored successfully",
        "modPath": normalized_mod_path.to_string_lossy()
    }))
}

/// Restore backups for multiple mods (optimized batch version)
#[command]
pub async fn restore_backups(
    app: AppHandle,
    mod_paths: Vec<String>,
    backup_directory: String,
) -> Result<serde_json::Value, String> {
    if mod_paths.is_empty() {
        return Ok(serde_json::json!({}));
    }
    
    // Restore all backups in parallel
    let mut restore_futures = Vec::new();
    
    for mod_path in mod_paths {
        let mod_path_clone = mod_path.clone();
        let backup_dir_clone = backup_directory.clone();
        let app_clone = app.clone();
        
        // Spawn restore task for each mod
        let future = async move {
            let result = restore_backup(app_clone, mod_path_clone.clone(), backup_dir_clone).await;
            (mod_path_clone, result)
        };
        
        restore_futures.push(future);
    }
    
    // Wait for all restores to complete
    let results = futures::future::join_all(restore_futures).await;
    
    // Build result map
    let mut result_map = serde_json::Map::new();
    for (mod_path, result) in results {
        match result {
            Ok(success_data) => {
                result_map.insert(mod_path, serde_json::json!({
                    "success": true,
                    "data": success_data
                }));
            }
            Err(error_msg) => {
                result_map.insert(mod_path, serde_json::json!({
                    "success": false,
                    "error": error_msg
                }));
            }
        }
    }
    
    Ok(serde_json::Value::Object(result_map))
}

