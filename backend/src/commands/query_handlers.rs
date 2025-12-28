// Mod query commands

use crate::core::mod_scanner::{query_mods_for_updates, BaseMod, update_mod_details as update_mod_details_query, list_installed_mods as list_installed_mods_query};
use crate::services::validate_mods_path;
use tauri::command;

/// Query mods folder for outdated mods
#[command]
pub async fn query_mods(
    mods_path: String,
    ignored_mods: Vec<String>,
) -> Result<Vec<BaseMod>, String> {
    let path = validate_mods_path(&mods_path)?;
    
    query_mods_for_updates(&path, &ignored_mods)
        .await
        .map_err(|e| format!("Failed to query mods: {}", e))
}

/// List all installed mods in mods folder (fast version - returns immediately with local data only)
#[command]
pub async fn list_installed_mods(
    mods_path: String,
) -> Result<Vec<BaseMod>, String> {
    let path = validate_mods_path(&mods_path)?;
    
    list_installed_mods_query(&path)
        .await
        .map_err(|e| format!("Failed to list installed mods: {}", e))
}

/// Update mod details from Steam API in background
/// This should be called after list_installed_mods to fetch details from API
#[command]
pub async fn update_mod_details(
    mods: Vec<BaseMod>,
) -> Result<Vec<BaseMod>, String> {
    update_mod_details_query(mods)
        .await
        .map_err(|e| format!("Failed to update mod details: {}", e))
}

