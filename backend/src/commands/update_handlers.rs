// Mod update commands

use std::path::PathBuf;
use std::collections::HashMap;
use tauri::{AppHandle, Emitter};
use crate::core::mod_scanner::BaseMod;
use crate::core::mod_manager::ModUpdater;
use crate::core::access_check::ensure_directory_access;
use crate::services::{get_downloader, get_mods_path_from_mod_path, find_all_mod_folders_with_id, write_last_updated_file, reset_update_cancel_flag, is_update_cancelled, cancel_update};

/// Cancel ongoing mod updates
#[tauri::command]
pub async fn cancel_update_mods(app: AppHandle) -> Result<(), String> {
    cancel_update();
    
    // Emit cancellation event for all active mods
    let _ = app.emit("update-cancelled", serde_json::json!({}));
    
    Ok(())
}

/// Check if update is cancelled
#[tauri::command]
pub async fn check_update_cancelled() -> Result<bool, String> {
    Ok(is_update_cancelled())
}

/// Reset the cancellation flag
#[tauri::command]
pub async fn reset_update_cancel_flag_command() -> Result<(), String> {
    reset_update_cancel_flag();
    Ok(())
}

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
    
    // Reset cancellation flag at the start of update
    reset_update_cancel_flag();
    
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
        // Check if update was cancelled
        if is_update_cancelled() {
            eprintln!("[UPDATE_MODS] Update cancelled by user");
            
            // Kill only our tracked SteamCMD processes
            {
                let downloader = get_downloader();
                let mut dl = downloader.lock().await;
                dl.kill_our_processes().await;
            }
            
            // Wait for all already-spawned installation tasks to complete
            // This prevents race conditions where mods are installed after cancellation
            let results: Vec<(String, Result<PathBuf, String>)> = futures::future::join_all(update_handles).await
                .into_iter()
                .map(|result: Result<(String, Result<PathBuf, String>), tokio::task::JoinError>| {
                    result.unwrap_or_else(|e| {
                        eprintln!("[UPDATE_MODS] Task panicked: {:?}", e);
                        ("".to_string(), Err(format!("Task panicked: {:?}", e)))
                    })
                })
                .collect();
            
            // Emit cancellation event for remaining mods
            for mod_id in &mod_ids {
                if !seen_mod_ids.contains(mod_id) {
                    let _ = app.emit("mod-state", serde_json::json!({
                        "modId": mod_id,
                        "state": "cancelled"
                    }));
                }
            }
            
            // Mark all mods as cancelled, but preserve any that completed before cancellation
            let mut cancelled_mods = Vec::new();
            for (mod_id, result) in results {
                // Skip entries with empty mod_id (indicates task panic where we lost mod_id)
                if mod_id.is_empty() {
                    eprintln!("[UPDATE_MODS] Skipping panicked task result - mod_id unknown");
                    continue;
                }

                match result {
                    Ok(_path) => {
                        // Mod was successfully updated before cancellation
                        if let Some(original_mod) = mods_map.get(&mod_id) {
                            let mut updated_mod = original_mod.clone();
                            updated_mod.updated = Some(true);
                            cancelled_mods.push(updated_mod);
                        }
                    }
                    Err(_) => {
                        // Mark as cancelled
                        if let Some(original_mod) = mods_map.get(&mod_id) {
                            let mut cancelled_mod = (*original_mod).clone();
                            cancelled_mod.updated = Some(false);
                            cancelled_mods.push(cancelled_mod);
                        }
                    }
                }
            }
            
            // Add mods that were queued but not started
            for mod_id in &mod_ids {
                if !seen_mod_ids.contains(mod_id) {
                    if let Some(original_mod) = mods_map.get(mod_id) {
                        let mut cancelled_mod = (*original_mod).clone();
                        cancelled_mod.updated = Some(false);
                        cancelled_mods.push(cancelled_mod);
                    }
                }
            }
            
            return Ok(cancelled_mods);
        }
        
        match result {
            Ok(downloaded_mod) => {
                // Check if update was cancelled before processing
                if is_update_cancelled() {
                    eprintln!("[UPDATE_MODS] Update cancelled, ignoring downloaded mod");
                    continue;
                }
                
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
                    // Check if cancelled before processing
                    if is_update_cancelled() {
                        eprintln!("[UPDATE_MODS] Update cancelled, skipping mod {}", mod_id);
                        let _ = app_clone.emit("mod-state", serde_json::json!({
                            "modId": mod_id,
                            "state": "cancelled"
                        }));
                        return (mod_id, Err("Update cancelled".to_string()));
                    }
                    
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
                // Check if error is due to cancellation
                if error_msg.contains("cancelled") || error_msg.contains("Update cancelled by user") {
                    eprintln!("[UPDATE_MODS] Download cancelled (not a failure): {}", error_msg);
                    // Don't treat cancellation as failure - it will be handled by cancellation check above
                    continue;
                }
                
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
    
    // Handle mods that failed to download (but not if cancelled)
    if !is_update_cancelled() {
        for failed_mod_id in &failed_download_mod_ids {
            eprintln!("[UPDATE_MODS] Mod {} failed to download", failed_mod_id);
            
            // Emit mod-updated event with failure
            let _ = app.emit("mod-updated", serde_json::json!({
                "modId": failed_mod_id,
                "success": false,
                "error": "Download failed - SteamCMD reported failure"
            }));
        }
    } else {
        // If cancelled, mark remaining mods as cancelled, not failed
        for failed_mod_id in &failed_download_mod_ids {
            eprintln!("[UPDATE_MODS] Mod {} download cancelled (not failed)", failed_mod_id);
            let _ = app.emit("mod-state", serde_json::json!({
                "modId": failed_mod_id,
                "state": "cancelled"
            }));
        }
    }
    
    // Check if cancelled before waiting for results
    if is_update_cancelled() {
        eprintln!("[UPDATE_MODS] Update cancelled, stopping wait for results");
        // Still wait for tasks to complete, but mark remaining as cancelled
        let results: Vec<_> = futures::future::join_all(update_handles).await
            .into_iter()
            .map(|result| {
                result.unwrap_or_else(|e| {
                    eprintln!("[UPDATE_MODS] Task panicked: {:?}", e);
                    ("".to_string(), Err(format!("Task panicked: {:?}", e)))
                })
            })
            .collect();
        
        let mut cancelled_mods = Vec::new();
        for (mod_id, result) in results {
            // Skip entries with empty mod_id (indicates task panic where we lost mod_id)
            if mod_id.is_empty() {
                eprintln!("[UPDATE_MODS] Skipping panicked task result - mod_id unknown");
                continue;
            }

            match result {
                Ok(_path) => {
                    // Mod was successfully updated before cancellation
                    if let Some(original_mod) = mods_map.get(&mod_id) {
                        let mut updated_mod = original_mod.clone();
                        updated_mod.updated = Some(true);
                        cancelled_mods.push(updated_mod);
                    }
                }
                Err(_) => {
                    // Mark as cancelled
                    if let Some(original_mod) = mods_map.get(&mod_id) {
                        let mut cancelled_mod = (*original_mod).clone();
                        cancelled_mod.updated = Some(false);
                        cancelled_mods.push(cancelled_mod);
                    }
                }
            }
        }
        
        // Add mods that were queued but not started
        for mod_id in &mod_ids {
            if !seen_mod_ids.contains(mod_id) {
                if let Some(original_mod) = mods_map.get(mod_id) {
                    let mut cancelled_mod = (*original_mod).clone();
                    cancelled_mod.updated = Some(false);
                    cancelled_mods.push(cancelled_mod);
                }
            }
        }
        
        return Ok(cancelled_mods);
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
        // Skip entries with empty mod_id (indicates task panic where we lost mod_id)
        if mod_id.is_empty() {
            eprintln!("[UPDATE_MODS] Skipping panicked task result - mod_id unknown");
            continue;
        }

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

