// Mod update commands

use std::path::PathBuf;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter};
use crate::core::mod_scanner::BaseMod;
use crate::core::mod_manager::ModUpdater;
use crate::services::{get_downloader, get_mods_path_from_mod_path, find_all_mod_folders_with_id, write_last_updated_file};

/// Update mods
#[tauri::command]
pub async fn update_mods(
    app: AppHandle,
    mods: Vec<BaseMod>,
    backup_mods: bool,
    backup_directory: Option<String>,
) -> Result<Vec<BaseMod>, String> {
    if mods.is_empty() {
        return Err("mods array is required".to_string());
    }
    
    // Extract modsPath from first mod
    let first_mod = &mods[0];
    let mods_path = get_mods_path_from_mod_path(&PathBuf::from(&first_mod.mod_path))?;
    
    // Prepare mods for download
    let mod_ids: Vec<String> = mods.iter().map(|m| m.mod_id.clone()).collect();
    
    // Build mod sizes map for load balancing
    let mod_sizes: HashMap<String, u64> = mods
        .iter()
        .filter_map(|m| {
            m.details.as_ref().map(|d| (m.mod_id.clone(), d.file_size))
        })
        .collect();
    
    // Download mods with size information for load balancing
    let downloader = get_downloader();
    let (downloaded_mods, download_path) = {
        let mut dl = downloader.lock().await;
        let download_path = dl.download_path().clone();
        let downloaded_mods = if mod_sizes.is_empty() {
            // No size information available, use simple download
            dl.download_mods(&mod_ids, Some(&app)).await
        } else {
            // Use size-based load balancing
            dl.download_mods_with_sizes(&mod_ids, Some(&mod_sizes), Some(&app)).await
        }
        .map_err(|e| format!("Failed to download mods: {}", e))?;
        (downloaded_mods, download_path)
    };
    
    if downloaded_mods.is_empty() {
        return Err("Failed to download any mods. Check SteamCMD logs for details.".to_string());
    }
    
    // Create HashMap for O(1) lookup instead of O(n) find()
    let mods_map: HashMap<String, &BaseMod> = mods.iter()
        .map(|m| (m.mod_id.clone(), m))
        .collect();
    
    // Update mods in parallel (different mods can be updated simultaneously)
    let mut update_futures = Vec::new();
    
    for downloaded_mod in downloaded_mods {
        let mod_id = downloaded_mod.mod_id.clone();
        let mod_path = downloaded_mod.mod_path.clone();
        let original_mod = mods_map.get(&mod_id)
            .ok_or_else(|| format!("Original mod not found for {}", mod_id))?;
        
        let existing_folder_name = original_mod.folder.as_deref();
        let mod_title = original_mod.details.as_ref().map(|d| d.title.clone());
        let remote_update_time = original_mod.details.as_ref()
            .map(|d| d.time_updated)
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as i64
            });
        
        let download_path_clone = download_path.clone();
        let mods_path_clone = mods_path.clone();
        let backup_dir_clone = backup_directory.as_ref().map(|s| PathBuf::from(s));
        let app_clone = app.clone();
        
        // Spawn update task for each mod
        let future = async move {
            eprintln!("[UPDATE_MODS] Processing downloaded mod: {} at {:?}", mod_id, mod_path);
            
            let updater = ModUpdater;
            let mod_path_result = updater.update_mod(
                &mod_id,
                &mod_path,
                &download_path_clone,
                &mods_path_clone,
                existing_folder_name,
                backup_mods,
                backup_dir_clone.as_deref(),
                mod_title.as_deref(),
            ).await;
            
            match mod_path_result {
                Ok(updated_path) => {
                    eprintln!("[UPDATE_MODS] Successfully updated mod {} to {:?}", mod_id, updated_path);
                    
                    // Find all folders with the same mod ID and update .lastupdated
                    let all_mod_folders = find_all_mod_folders_with_id(&mods_path_clone, &mod_id)
                        .await
                        .unwrap_or_default();
                    
                    // Update .lastupdated files in parallel
                    let mut update_file_futures = Vec::new();
                    for folder_path in all_mod_folders {
                        update_file_futures.push(write_last_updated_file(folder_path, remote_update_time));
                    }
                    
                    // Wait for all .lastupdated files to be written
                    futures::future::join_all(update_file_futures).await;
                    
                    // Emit event for successfully updated mod
                    let _ = app_clone.emit("mod-updated", serde_json::json!({
                        "modId": mod_id,
                        "success": true,
                    }));
                    
                    (mod_id, Ok(updated_path))
                }
                Err(e) => {
                    eprintln!("[UPDATE_MODS] Error updating mod {}: {}", mod_id, e);
                    
                    // Emit event for failed mod update
                    let _ = app_clone.emit("mod-updated", serde_json::json!({
                        "modId": mod_id,
                        "success": false,
                        "error": e.to_string(),
                    }));
                    
                    (mod_id, Err(e))
                }
            }
        };
        
        update_futures.push(future);
    }
    
    // Wait for all updates to complete
    let results = futures::future::join_all(update_futures).await;
    let mut updated_mods = Vec::new();
    
    for (mod_id, result) in results {
        match result {
            Ok(_path) => {
                // Find the original mod to return
                if let Some(original_mod) = mods_map.get(&mod_id) {
                    let mut updated_mod = (*original_mod).clone();
                    updated_mod.updated = Some(true);
                    updated_mods.push(updated_mod);
                }
            }
            Err(e) => {
                eprintln!("[UPDATE_MODS] Failed to update mod {}: {}", mod_id, e);
                // Still add the mod but mark as not updated
                if let Some(original_mod) = mods_map.get(&mod_id) {
                    let mut failed_mod = (*original_mod).clone();
                    failed_mod.updated = Some(false);
                    updated_mods.push(failed_mod);
                }
            }
        }
    }
    
    Ok(updated_mods)
}

