// Download-related commands

use std::path::PathBuf;
use serde_json;
use tauri::{command, AppHandle, Emitter};
use crate::core::mod_manager::ModUpdater;
use crate::core::mod_scanner::query_mod_batch;
use crate::core::access_check::ensure_directory_access;
use crate::services::{get_downloader, get_steam_api, write_last_updated_file};

/// Download mod(s) from Steam Workshop
#[command]
pub async fn download_mod(
    app: AppHandle,
    mod_id: String,
    _title: Option<String>,
    mods_path: String,
    max_steamcmd_instances: Option<usize>,
) -> Result<serde_json::Value, String> {
    // Check if mod is already downloading
    {
        let downloader = get_downloader();
        let dl = downloader.lock().await;
        if dl.is_downloading(&mod_id) {
            return Err("Mod is already being downloaded".to_string());
        }
    }
    
    // Check directory access before proceeding
    let mods_path_buf = PathBuf::from(&mods_path);
    ensure_directory_access(&app, &mods_path_buf, &mods_path)?;
    
    // Mark as downloading
    {
        let downloader = get_downloader();
        let mut dl = downloader.lock().await;
        dl.mark_downloading(mod_id.clone());
    }
    
    // Download mod
    let mod_id_for_download = mod_id.clone();
    let downloader_for_download = get_downloader();
    let mut dl_guard = downloader_for_download.lock().await;
    let mod_receiver_result = dl_guard.download_mods(&[mod_id_for_download], Some(&app), max_steamcmd_instances).await;
    drop(dl_guard); // Release lock before await
    
    let mut mod_receiver = match mod_receiver_result {
        Ok(receiver) => receiver,
        Err(e) => {
            // Cleanup on error
            let downloader_cleanup = get_downloader();
            let mut dl_cleanup = downloader_cleanup.lock().await;
            dl_cleanup.mark_downloaded(&mod_id);
            drop(dl_cleanup);
            return Err(format!("Failed to download mod: {}", e));
        }
    };
    
    // Wait for the mod to be downloaded
    let downloaded_mod = match mod_receiver.recv().await {
        Some(Ok(mod_info)) => mod_info,
        Some(Err(e)) => {
            let downloader = get_downloader();
            let mut dl = downloader.lock().await;
            dl.mark_downloaded(&mod_id);
            drop(dl);
            return Err(format!("Mod download failed: {}", e));
        }
        None => {
            let downloader = get_downloader();
            let mut dl = downloader.lock().await;
            dl.mark_downloaded(&mod_id);
            drop(dl);
            return Err("Mod download completed but no mod folder was created".to_string());
        }
    };
    
    // Emit installing event before copying
    let _ = app.emit("mod-state", serde_json::json!({
        "modId": mod_id,
        "state": "installing"
    }));
    
    // Copy mod to mods folder
    let updater = ModUpdater;
    let downloader_for_path = get_downloader();
    let download_path = {
        let dl = downloader_for_path.lock().await;
        dl.download_path().clone()
    };
    let mods_path_buf = PathBuf::from(&mods_path);
    
    // Get mod details to retrieve title and time_updated (use batch query for efficiency)
    let (mod_title, time_updated) = match query_mod_batch(&[mod_id.clone()], 0).await {
        Ok(mut details) => {
            if let Some(detail) = details.pop() {
                (Some(detail.title.clone()), detail.time_updated)
            } else {
                // Fallback to SteamApi if batch query returns no results
                let steam_api = get_steam_api();
                let mut api = steam_api.lock().await;
                match api.get_file_details(&mod_id).await {
                    Ok(details) => (Some(details.title.clone()), details.time_updated),
                    Err(_) => {
                        (None, std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as i64)
                    }
                }
            }
        }
        Err(_) => {
            // Fallback to SteamApi if batch query fails
            let steam_api = get_steam_api();
            let mut api = steam_api.lock().await;
            match api.get_file_details(&mod_id).await {
                Ok(details) => (Some(details.title.clone()), details.time_updated),
                Err(_) => {
                    (None, std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs() as i64)
                }
            }
        }
    };
    
    let mod_path_result = updater.update_mod(
        &downloaded_mod.mod_id,
        &downloaded_mod.mod_path,
        &download_path,
        &mods_path_buf,
        None,
        false,
        None,
        mod_title.as_deref(),
    ).await;
    
    let mod_id_for_cleanup = mod_id.clone();
    let mod_path = match mod_path_result {
        Ok(path) => path,
        Err(e) => {
            // Cleanup on error
            let downloader_cleanup = get_downloader();
            let mut dl_cleanup = downloader_cleanup.lock().await;
            dl_cleanup.mark_downloaded(&mod_id_for_cleanup);
            drop(dl_cleanup);
            return Err(format!("Failed to update mod: {}", e));
        }
    };
    
    // Create .lastupdated file
    write_last_updated_file(mod_path.clone(), time_updated).await;
    
    // Mark as downloaded
    {
        let downloader_final = get_downloader();
        let mut dl_final = downloader_final.lock().await;
        dl_final.mark_downloaded(&mod_id);
        drop(dl_final);
    }
    
    // Emit mod-state: completed event
    let _ = app.emit("mod-state", serde_json::json!({
        "modId": mod_id,
        "state": "completed"
    }));
    
    // Emit mod-updated event to notify frontend that mod was successfully downloaded and installed
    let _ = app.emit("mod-updated", serde_json::json!({
        "modId": mod_id,
        "success": true,
    }));
    
    Ok(serde_json::json!({
        "modId": downloaded_mod.mod_id,
        "modPath": mod_path.to_string_lossy(),
        "folder": downloaded_mod.folder,
    }))
}

