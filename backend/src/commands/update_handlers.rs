// Mod update commands

use std::path::PathBuf;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter};
use crate::core::mod_scanner::BaseMod;
use crate::core::mod_manager::ModUpdater;
use crate::core::access_check::ensure_directory_access;
use crate::services::{get_downloader, get_mods_path_from_mod_path, find_all_mod_folders_with_id, write_last_updated_file};

/// Update mods
#[tauri::command]
pub async fn update_mods(
    app: AppHandle,
    mods: Vec<BaseMod>,
    backup_mods: bool,
    backup_directory: Option<String>,
    max_steamcmd_instances: Option<usize>,
) -> Result<Vec<BaseMod>, String> {
    if mods.is_empty() {
        return Err("mods array is required".to_string());
    }
    
    // Filter out non-Steam mods - they can't be updated from Workshop
    let steam_mods: Vec<BaseMod> = mods.into_iter()
        .filter(|m| !m.non_steam_mod)
        .collect();
    
    if steam_mods.is_empty() {
        return Err("No Steam Workshop mods to update. Non-Steam mods cannot be updated.".to_string());
    }
    
    // Extract modsPath from first mod
    let first_mod = &steam_mods[0];
    let mods_path = get_mods_path_from_mod_path(&PathBuf::from(&first_mod.mod_path))?;
    let mods_path_str = mods_path.to_string_lossy().to_string();
    
    // Check directory access before proceeding
    ensure_directory_access(&app, &mods_path, &mods_path_str)?;
    
    // Prepare mods for download
    let mod_ids: Vec<String> = steam_mods.iter().map(|m| m.mod_id.clone()).collect();
    
    // Build mod sizes map for load balancing
    let mod_sizes: HashMap<String, u64> = steam_mods
        .iter()
        .filter_map(|m| {
            m.details.as_ref().map(|d| (m.mod_id.clone(), d.file_size))
        })
        .collect();
    
    // Download mods with size information for load balancing
    // This now returns a channel that yields mods as they are downloaded
    let downloader = get_downloader();
    let (mut mod_receiver, download_path) = {
        let mut dl = downloader.lock().await;
        let download_path = dl.download_path().clone();
        let mod_receiver_result = if mod_sizes.is_empty() {
            // No size information available, use simple download
            dl.download_mods(&mod_ids, Some(&app), max_steamcmd_instances).await
        } else {
            // Use size-based load balancing
            dl.download_mods_with_sizes(&mod_ids, Some(&mod_sizes), Some(&app), max_steamcmd_instances).await
        };
        
        match mod_receiver_result {
            Ok(mod_receiver) => (mod_receiver, download_path),
            Err(e) => {
                // If download completely failed, return error
                return Err(format!("Failed to download mods: {}", e));
            }
        }
    };
    
    // Create HashMap for O(1) lookup instead of O(n) find()
    // Clone the mods data so we can move it into spawned tasks
    let mods_map: HashMap<String, BaseMod> = steam_mods.iter()
        .map(|m| (m.mod_id.clone(), m.clone()))
        .collect();
    
    // Track which mods we've seen (for detecting failures)
    let mut seen_mod_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut update_handles = Vec::new();
    
    // Process mods as they arrive from the channel
    // This allows installation to start immediately after each mod is downloaded,
    // without waiting for all downloads to complete
    while let Some(result) = mod_receiver.recv().await {
        match result {
            Ok(downloaded_mod) => {
                // Mark this mod as seen
                seen_mod_ids.insert(downloaded_mod.mod_id.clone());
                
                // Emit installing state immediately
                let _ = app.emit("mod-state", serde_json::json!({
                    "modId": downloaded_mod.mod_id,
                    "state": "installing"
                }));
                
                let mod_id = downloaded_mod.mod_id.clone();
                let mod_path = downloaded_mod.mod_path.clone();
                let original_mod = mods_map.get(&mod_id)
                    .ok_or_else(|| format!("Original mod not found for {}", mod_id))?;
                
                let existing_folder_name = original_mod.folder.clone();
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
                
                // Spawn independent task for each mod installation
                // This ensures events are emitted immediately when each mod completes
                let handle = tokio::spawn(async move {
                    eprintln!("[UPDATE_MODS] Processing downloaded mod: {} at {:?}", mod_id, mod_path);
                                
                    let updater = ModUpdater;
                    let mod_path_result = updater.update_mod(
                        &mod_id,
                        &mod_path,
                        &download_path_clone,
                        &mods_path_clone,
                        existing_folder_name.as_deref(),
                        backup_mods,
                        backup_dir_clone.as_deref(),
                        mod_title.as_deref(),
                        None, // force_overwrite_corrupted - None means ask user if corrupted mod found
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
                    
                    // Emit "completed" state event IMMEDIATELY
                    // This marks the mod as completed in the UI
                    let _ = app_clone.emit("mod-state", serde_json::json!({
                        "modId": mod_id,
                        "state": "completed",
                    }));
                    
                    // Emit mod-updated event for backward compatibility and final status
                    let _ = app_clone.emit("mod-updated", serde_json::json!({
                        "modId": mod_id,
                        "success": true,
                    }));
                    
                    (mod_id, Ok(updated_path))
                }
                Err(e) => {
                    eprintln!("[UPDATE_MODS] Error updating mod {}: {}", mod_id, e);
                    
                    // Emit event for failed mod update IMMEDIATELY
                    let _ = app_clone.emit("mod-updated", serde_json::json!({
                        "modId": mod_id,
                        "success": false,
                        "error": e.to_string(),
                    }));
                    
                    (mod_id, Err(e))
                }
            }
        });
        
        update_handles.push(handle);
            }
            Err(error_msg) => {
                // Handle download failure - extract mod ID from error message if possible
                eprintln!("[UPDATE_MODS] Download channel reported error: {}", error_msg);
                // Don't emit mod-updated here - let the retry system handle state transitions
                // The retry system will emit "retry-queued" or "failed" as appropriate
                // We just log the error here
            }
        }
    }
    
    // Find mods that failed to download (mods that were requested but never seen)
    let failed_download_mod_ids: Vec<String> = mod_ids
        .iter()
        .filter(|id| !seen_mod_ids.contains(*id))
        .cloned()
        .collect();
    
    // Handle mods that failed to download
    for failed_mod_id in &failed_download_mod_ids {
        eprintln!("[UPDATE_MODS] Mod {} failed to download", failed_mod_id);
        
        // Emit mod-updated event with failure
        let _ = app.emit("mod-updated", serde_json::json!({
            "modId": failed_mod_id,
            "success": false,
            "error": "Download failed - SteamCMD reported failure"
        }));
    }
    
    // Wait for all updates to complete
    // Note: Each task emits events independently, so frontend receives them immediately
    let results: Vec<_> = futures::future::join_all(update_handles).await
        .into_iter()
        .map(|result| {
            // Handle task join errors
            result.unwrap_or_else(|e| {
                eprintln!("[UPDATE_MODS] Task panicked: {:?}", e);
                ("".to_string(), Err(format!("Task panicked: {:?}", e)))
            })
        })
        .collect();
    let mut updated_mods = Vec::new();
    
    for (mod_id, result) in results {
        match result {
            Ok(_path) => {
                // Find the original mod to return
                if let Some(original_mod) = mods_map.get(&mod_id) {
                    let mut updated_mod = original_mod.clone();
                    updated_mod.updated = Some(true);
                    updated_mods.push(updated_mod);
                }
            }
            Err(e) => {
                eprintln!("[UPDATE_MODS] Failed to update mod {}: {}", mod_id, e);
                // Still add the mod but mark as not updated
                if let Some(original_mod) = mods_map.get(&mod_id) {
                    let mut failed_mod = original_mod.clone();
                    failed_mod.updated = Some(false);
                    updated_mods.push(failed_mod);
                }
            }
        }
    }
    
    // Add mods that failed to download to the result
    for failed_mod_id in &failed_download_mod_ids {
        if let Some(original_mod) = mods_map.get(failed_mod_id) {
            let mut failed_mod = (*original_mod).clone();
            failed_mod.updated = Some(false);
            updated_mods.push(failed_mod);
        }
    }
    
    Ok(updated_mods)
}

