// Ignore-related commands

use std::path::PathBuf;
use serde_json;
use tauri::command;
use crate::core::mod_scanner::BaseMod;
use crate::services::{find_all_mod_folders_with_id, fetch_mod_times_updated, write_ignore_update_file, get_mods_path_from_mod_path};

/// Ignore this update - create .ignoredupdate file with current remote timestamp
#[command]
pub async fn ignore_update(
    mods: Vec<BaseMod>,
) -> Result<Vec<serde_json::Value>, String> {
    if mods.is_empty() {
        return Ok(vec![]);
    }
    
    // Separate mods with and without details
    let (mods_with_details, mods_without_details): (Vec<_>, Vec<_>) = mods
        .into_iter()
        .partition(|m| m.details.is_some());
    
    // Fetch details for mods without them
    let mod_ids_without_details: Vec<String> = mods_without_details
        .iter()
        .map(|m| m.mod_id.clone())
        .collect();
    
    let mod_id_to_time_updated = if !mod_ids_without_details.is_empty() {
        fetch_mod_times_updated(&mod_ids_without_details).await
    } else {
        std::collections::HashMap::new()
    };
    
    // Process all mods in parallel
    let mut ignore_futures = Vec::new();
    
    // Helper function to process a single mod
    async fn process_ignore_mod(
        mod_id: String,
        mod_path: String,
        time_updated: i64,
    ) -> Result<String, String> {
        let mod_path_buf = PathBuf::from(&mod_path);
        let mods_path = get_mods_path_from_mod_path(&mod_path_buf)?;
        
        let all_mod_folders = find_all_mod_folders_with_id(&mods_path, &mod_id)
            .await
            .unwrap_or_default();
        
        // Create .ignoredupdate files in parallel
        let mut file_futures = Vec::new();
        for folder_path in all_mod_folders {
            file_futures.push(write_ignore_update_file(folder_path, time_updated));
        }
        
        futures::future::join_all(file_futures).await;
        
        Ok(mod_id)
    }
    
    // Process mods with details
    for mod_ref in mods_with_details {
        let mod_id = mod_ref.mod_id.clone();
        let mod_path = mod_ref.mod_path.clone();
        let time_updated = mod_ref.details.as_ref().unwrap().time_updated;
        
        ignore_futures.push((mod_id.clone(), process_ignore_mod(mod_id, mod_path, time_updated)));
    }
    
    // Process mods without details
    for mod_ref in mods_without_details {
        let mod_id = mod_ref.mod_id.clone();
        let mod_path = mod_ref.mod_path.clone();
        let time_updated = mod_id_to_time_updated
            .get(&mod_id)
            .copied()
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64
            });
        
        ignore_futures.push((mod_id.clone(), process_ignore_mod(mod_id, mod_path, time_updated)));
    }
    
    // Wait for all ignores to complete
    let results = futures::future::join_all(
        ignore_futures.into_iter().map(|(mod_id, future)| async move {
            (mod_id, future.await)
        })
    ).await;
    
    let mut ignored_mods = Vec::new();
    for (mod_id, _result) in results {
        // Always mark as ignored, even if file write failed
        ignored_mods.push(serde_json::json!({
            "modId": mod_id,
            "ignored": true
        }));
    }
    
    Ok(ignored_mods)
}

/// Check if mods have .ignoredupdate file (ignored updates)
#[command]
pub async fn check_ignored_updates(
    mod_paths: Vec<String>,
) -> Result<serde_json::Value, String> {
    if mod_paths.is_empty() {
        return Ok(serde_json::json!({}));
    }
    
    // Check all mods in parallel
    let mut check_futures = Vec::new();
    
    for mod_path in mod_paths {
        let mod_path_buf = PathBuf::from(&mod_path);
        let about_path = mod_path_buf.join("About");
        let ignore_update_path = about_path.join(".ignoredupdate");
        let mod_path_clone = mod_path.clone();
        
        // Spawn blocking task for each check
        let future = tokio::task::spawn_blocking(move || {
            ignore_update_path.exists()
        });
        
        check_futures.push((mod_path_clone, future));
    }
    
    // Wait for all checks to complete and build result map
    let mut results = serde_json::Map::new();
    for (mod_path, future) in check_futures {
        match future.await {
            Ok(has_ignored) => {
                results.insert(mod_path, serde_json::json!({
                    "hasIgnoredUpdate": has_ignored
                }));
            }
            Err(_) => {
                results.insert(mod_path, serde_json::json!({
                    "hasIgnoredUpdate": false
                }));
            }
        }
    }
    
    Ok(serde_json::Value::Object(results))
}

/// Undo ignore this update - remove .ignoredupdate file to allow updates again
#[command]
pub async fn undo_ignore_update(
    mods: Vec<BaseMod>,
) -> Result<Vec<serde_json::Value>, String> {
    if mods.is_empty() {
        return Ok(vec![]);
    }
    
    // Process all mods in parallel
    let mut undo_futures = Vec::new();
    
    for mod_ref in mods {
        let mod_id = mod_ref.mod_id.clone();
        let mod_path = PathBuf::from(&mod_ref.mod_path);
        let mods_path = get_mods_path_from_mod_path(&mod_path)?;
        
        let mod_id_clone = mod_id.clone();
        let mods_path_clone = mods_path.clone();
        
        // Spawn task to remove .ignoredupdate files
        let future = async move {
            let all_mod_folders = find_all_mod_folders_with_id(&mods_path_clone, &mod_id_clone)
                .await
                .unwrap_or_default();
            
            // Remove .ignoredupdate files in parallel
            let mut file_futures = Vec::new();
            for folder_path in all_mod_folders {
                let about_path = folder_path.join("About");
                let ignore_update_path = about_path.join(".ignoredupdate");
                
                let future = tokio::task::spawn_blocking(move || {
                    if ignore_update_path.exists() {
                        if let Err(e) = std::fs::remove_file(&ignore_update_path) {
                            eprintln!("Failed to remove .ignoredupdate file: {}", e);
                        }
                    }
                });
                file_futures.push(future);
            }
            
            futures::future::join_all(file_futures).await;
            mod_id_clone
        };
        
        undo_futures.push((mod_id, future));
    }
    
    // Wait for all undo operations to complete
    let results = futures::future::join_all(
        undo_futures.into_iter().map(|(mod_id, future)| async move {
            (mod_id, future.await)
        })
    ).await;
    
    let mut undone_mods = Vec::new();
    for (mod_id, _) in results {
        undone_mods.push(serde_json::json!({
            "modId": mod_id,
            "undone": true
        }));
    }
    
    Ok(undone_mods)
}

