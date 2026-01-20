// Export commands

use crate::core::mod_scanner::{BaseMod, list_installed_mods as list_installed_mods_query, update_mod_details as update_mod_details_query};
use crate::core::access_check::check_directory_access_with_warning;
use crate::services::validate_mods_path;
use tauri::{command, AppHandle};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Export mod list to clipboard
/// Copies formatted string with one mod per line to clipboard
/// Uses Steam title if available, otherwise uses folder name
/// If mods are provided, uses them directly (they're already in memory with details)
/// Otherwise, fetches mods from mods_path
#[command]
pub async fn export_mods_to_clipboard(
    app: AppHandle,
    mods: Option<Vec<BaseMod>>,
    mods_path: Option<String>,
) -> Result<(), String> {
    let mods = if let Some(provided_mods) = mods {
        // Use mods provided by frontend (already loaded and have Steam details)
        provided_mods
    } else if let Some(mods_path_str) = mods_path {
        // Backend needs to fetch mods itself
        let path = validate_mods_path(&mods_path_str)?;
        
        // Check directory access (read access is required)
        check_directory_access_with_warning(&app, &path, &mods_path_str)?;
        
        // List installed mods
        let mut fetched_mods = list_installed_mods_query(&path)
            .await
            .map_err(|e| format!("Failed to list installed mods: {}", e))?;
        
        // Update mod details to get Steam titles
        fetched_mods = update_mod_details_query(fetched_mods)
            .await
            .map_err(|e| format!("Failed to update mod details: {}", e))?;
        
        fetched_mods
    } else {
        return Err("Either mods or mods_path must be provided".to_string());
    };
        
    let header = format!(
        "Created with Rimworld Workshop Downloader {}\nTotal # of mods: {}\n\n",
        env!("CARGO_PKG_VERSION"),
        mods.len()
    );
    
    // Format mod list in RimSort-compatible format: {name} [{package_id}][{url}]
    let mut lines = Vec::new();
    
    for mod_item in mods {
        // Get mod name
        let name = if let Some(details) = &mod_item.details {
            // Use Steam title if available
            details.title.clone()
        } else if let Some(folder) = &mod_item.folder {
            // Use folder name if no Steam details
            folder.clone()
        } else {
            // Fallback to mod_id if neither is available
            mod_item.mod_id.clone()
        };
        
        // Get package_id (mod_id - Steam ID or folder name)
        let package_id = mod_item.mod_id.clone();
        
        // Get URL - construct from publishedfileid if workshop_file_url is empty
        let url = if let Some(details) = &mod_item.details {
            if !details.workshop_file_url.is_empty() {
                // Use workshop_file_url if available
                details.workshop_file_url.clone()
            } else if !details.publishedfileid.is_empty() {
                // Construct URL from publishedfileid for Steam mods
                format!("https://steamcommunity.com/sharedfiles/filedetails/?id={}", details.publishedfileid)
            } else {
                String::new()
            }
        } else if !mod_item.non_steam_mod && !mod_item.mod_id.is_empty() {
            // For Steam mods without details, construct URL from mod_id
            format!("https://steamcommunity.com/sharedfiles/filedetails/?id={}", mod_item.mod_id)
        } else {
            // Non-Steam mods don't have URLs
            String::new()
        };
        
        // Format: {name} [{package_id}][{url}]
        if url.is_empty() {
            lines.push(format!("{} [{}]", name, package_id));
        } else {
            lines.push(format!("{} [{}][{}]", name, package_id, url));
        }
    }

    let mods_text = lines.join("\n");
    let formatted_text = format!("{}{}", header, mods_text);
    
    println!("Exported mod list: {:?}", lines);
    
    // Copy to clipboard using Tauri plugin
    app.clipboard()
        .write_text(formatted_text)
        .map_err(|e| format!("Failed to copy to clipboard: {}", e))?;
    
    Ok(())
}
