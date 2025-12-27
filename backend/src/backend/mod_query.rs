use std::path::{Path, PathBuf};
use std::fs;
use serde::{Deserialize, Serialize};
use crate::backend::deserializers::{bool_from_int, u64_from_str_or_int, i64_from_str_or_int, i32_from_str_or_int};

// Default value helpers for optional fields
fn default_i32() -> i32 {
    0
}

fn default_bool() -> bool {
    false
}

fn default_string() -> String {
    String::new()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BaseMod {
    pub mod_id: String,
    pub mod_path: String,
    pub folder: Option<String>,
    pub details: Option<WorkshopFileDetails>,
    pub updated: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkshopFileDetails {
    #[serde(default = "default_string")]
    pub publishedfileid: String,
    #[serde(deserialize_with = "i32_from_str_or_int", default = "default_i32")]
    pub result: i32,
    pub creator: String,
    #[serde(deserialize_with = "i32_from_str_or_int")]
    pub creator_app_id: i32,
    #[serde(deserialize_with = "i32_from_str_or_int")]
    pub consumer_app_id: i32,
    pub filename: String,
    #[serde(deserialize_with = "u64_from_str_or_int")]
    pub file_size: u64,
    pub file_url: String,
    pub hcontent_file: String,
    pub preview_url: String,
    pub hcontent_preview: String,
    pub title: String,
    pub description: String,
    #[serde(deserialize_with = "i64_from_str_or_int")]
    pub time_created: i64,
    #[serde(deserialize_with = "i64_from_str_or_int")]
    pub time_updated: i64,
    #[serde(deserialize_with = "i32_from_str_or_int")]
    pub visibility: i32,
    #[serde(deserialize_with = "i32_from_str_or_int", default = "default_i32")]
    pub flags: i32,
    #[serde(default = "default_string")]
    pub workshop_file_url: String,
    #[serde(deserialize_with = "bool_from_int", default = "default_bool")]
    pub workshop_accepted: bool,
    #[serde(deserialize_with = "bool_from_int", default = "default_bool")]
    pub show_subscribe_all: bool,
    #[serde(deserialize_with = "i32_from_str_or_int", default = "default_i32")]
    pub num_comments_developer: i32,
    #[serde(deserialize_with = "i32_from_str_or_int", default = "default_i32")]
    pub num_comments_public: i32,
    #[serde(deserialize_with = "bool_from_int", default = "default_bool")]
    pub banned: bool,
    #[serde(default = "default_string")]
    pub ban_reason: String,
    #[serde(default = "default_string")]
    pub banner: String,
    #[serde(deserialize_with = "bool_from_int", default = "default_bool")]
    pub can_be_deleted: bool,
    #[serde(default = "default_string")]
    pub app_name: String,
    #[serde(deserialize_with = "i32_from_str_or_int", default = "default_i32")]
    pub file_type: i32,
    #[serde(deserialize_with = "bool_from_int", default = "default_bool")]
    pub can_subscribe: bool,
    #[serde(deserialize_with = "i32_from_str_or_int", default = "default_i32")]
    pub subscriptions: i32,
    #[serde(deserialize_with = "i32_from_str_or_int", default = "default_i32")]
    pub favorited: i32,
    #[serde(deserialize_with = "i32_from_str_or_int", default = "default_i32")]
    pub followers: i32,
    #[serde(deserialize_with = "i32_from_str_or_int", default = "default_i32")]
    pub lifetime_subscriptions: i32,
    #[serde(deserialize_with = "i32_from_str_or_int", default = "default_i32")]
    pub lifetime_favorited: i32,
    #[serde(deserialize_with = "i32_from_str_or_int", default = "default_i32")]
    pub lifetime_followers: i32,
    #[serde(default = "default_string")]
    pub lifetime_playtime: String,
    #[serde(default = "default_string")]
    pub lifetime_playtime_sessions: String,
    #[serde(deserialize_with = "i32_from_str_or_int", default = "default_i32")]
    pub views: i32,
    #[serde(deserialize_with = "i32_from_str_or_int", default = "default_i32")]
    pub num_children: i32,
    #[serde(deserialize_with = "i32_from_str_or_int", default = "default_i32")]
    pub num_reports: i32,
    #[serde(default)]
    pub tags: Vec<Tag>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub tag: String,
}

/// Query mod ID from mod folder by reading PublishedFileId.txt
pub fn query_mod_id(mod_path: &Path) -> Result<Option<String>, Box<dyn std::error::Error>> {
    let about_path = mod_path.join("About");
    
    // Check if About folder exists and is a directory
    let about_metadata = match fs::metadata(&about_path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None); // Not a mod folder
        }
        Err(_e) => {
            return Ok(None);
        }
    };
    
    if !about_metadata.is_dir() {
        return Ok(None); // Not a mod folder
    }
    
    let file_id_path = about_path.join("PublishedFileId.txt");
    
    // Check if PublishedFileId.txt exists
    match fs::metadata(&file_id_path) {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None); // Not a workshop mod
        }
        Err(_e) => {
            return Ok(None);
        }
    }
    
    // Read and parse mod ID
    match fs::read_to_string(&file_id_path) {
        Ok(content) => {
            let file_id = content.trim();
            if file_id.is_empty() {
                return Ok(None);
            }
            Ok(Some(file_id.to_string()))
        }
        Err(_e) => {
            Ok(None)
        }
    }
}

/// Check if mod has ignored update (has .ignoredupdate file)
/// Returns the timestamp from .ignoredupdate file if it exists
pub fn get_ignored_update_timestamp(mod_path: &Path) -> Result<Option<i64>, Box<dyn std::error::Error>> {
    let about_path = mod_path.join("About");
    let ignore_update_path = about_path.join(".ignoredupdate");
    
    match fs::read_to_string(&ignore_update_path) {
        Ok(content) => {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                match trimmed.parse::<i64>() {
                    Ok(timestamp) if timestamp > 0 => {
                        return Ok(Some(timestamp));
                    }
                    Ok(_) | Err(_) => {
                        // Invalid timestamp, remove the file
                        let _ = fs::remove_file(&ignore_update_path);
                        return Ok(None);
                    }
                }
            }
            Ok(None)
        }
        Err(_e) if _e.kind() == std::io::ErrorKind::NotFound => {
            Ok(None)
        }
        Err(e) => {
            Err(format!("Failed to read .ignoredupdate file: {}", e).into())
        }
    }
}

/// Get mod's last updated time
/// Checks for .lastupdated file first, then falls back to PublishedFileId.txt creation time
pub fn get_mod_last_updated_time(mod_path: &Path) -> Result<std::time::SystemTime, Box<dyn std::error::Error>> {
    let about_path = mod_path.join("About");
    let last_updated_path = about_path.join(".lastupdated");
    
    // Check for .lastupdated timestamp file
    match fs::read_to_string(&last_updated_path) {
        Ok(content) => {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                match trimmed.parse::<i64>() {
                    Ok(timestamp) if timestamp > 0 => {
                        let duration = std::time::Duration::from_secs(timestamp as u64);
                        let datetime = std::time::UNIX_EPOCH + duration;
                        return Ok(datetime);
                    }
                    Ok(_) | Err(_) => {
                        let _ = fs::remove_file(&last_updated_path);
                    }
                }
            }
        }
        Err(_e) if _e.kind() != std::io::ErrorKind::NotFound => {
            // Error reading file, continue to fallback
        }
        _ => {}
    }
    
    // Fallback: use PublishedFileId.txt creation time
    let file_id_path = about_path.join("PublishedFileId.txt");
    match fs::metadata(&file_id_path) {
        Ok(metadata) => {
            // Use creation time if available, otherwise modification time
            // Use modification time (creation time is not reliably available on all platforms)
            Ok(metadata.modified().unwrap_or_else(|_| std::time::SystemTime::now()))
        }
        Err(_) => {
            // If PublishedFileId.txt doesn't exist, use mod folder's modification time
            let metadata = fs::metadata(mod_path)?;
            Ok(metadata.modified().unwrap_or_else(|_| std::time::SystemTime::now()))
        }
    }
}

/// Query batch of mods from Steam Workshop API
pub async fn query_mod_batch(
    mod_ids: &[String],
    retries: u32,
) -> Result<Vec<WorkshopFileDetails>, Box<dyn std::error::Error + Send + Sync>> {
    const STEAM_API_BASE: &str = "http://api.steampowered.com";
    const MAX_RETRIES: u32 = 3;
    const USER_AGENT: &str = "RimworldWorkshopDownloader/1.0";

    let url = format!("{}/ISteamRemoteStorage/GetPublishedFileDetails/v0001/", STEAM_API_BASE);
    let client = reqwest::Client::new();
    
    // Remove duplicates
    let unique_ids: Vec<String> = mod_ids.iter()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .cloned()
        .collect();
    
    // Build form parameters
    let mut params = std::collections::HashMap::new();
    params.insert("itemcount".to_string(), unique_ids.len().to_string());
    params.insert("format".to_string(), "json".to_string());
    
    for (index, id) in unique_ids.iter().enumerate() {
        params.insert(format!("publishedfileids[{}]", index), id.clone());
    }

    match client
        .post(&url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("User-Agent", USER_AGENT)
        .header("Accept", "application/json")
        .form(&params)
        .send()
        .await
    {
        Ok(response) => {
            if !response.status().is_success() {
            if retries < MAX_RETRIES {
                tokio::time::sleep(tokio::time::Duration::from_secs(1 * (retries + 1) as u64)).await;
                return Box::pin(query_mod_batch(mod_ids, retries + 1)).await;
                } else {
                    return Err(format!("Steam API error: {}", response.status()).into());
                }
            }
            
            let data: serde_json::Value = response.json().await?;
            
            let details = data["response"]["publishedfiledetails"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            
            if retries > 0 {
                eprintln!("Got batch of {} mods successfully after {} retries.", mod_ids.len(), retries);
            }
            
            let mut result = Vec::new();
            for detail in details {
                if let Ok(file_detail) = serde_json::from_value::<WorkshopFileDetails>(detail) {
                    result.push(file_detail);
                }
            }
            
            Ok(result)
        }
        Err(e) => {
            if retries < MAX_RETRIES {
                tokio::time::sleep(tokio::time::Duration::from_secs(1 * (retries + 1) as u64)).await;
                Box::pin(query_mod_batch(mod_ids, retries + 1)).await
            } else {
                Err(e.into())
            }
        }
    }
}

/// Query all mods in mods folder and check for updates
pub async fn query_mods_for_updates(
    mods_path: &Path,
    ignored_mods: &[String],
) -> Result<Vec<BaseMod>, Box<dyn std::error::Error>> {
    // Convert ignored_mods to HashSet for O(1) lookup
    let ignored_set: std::collections::HashSet<String> = ignored_mods.iter().cloned().collect();
    // Check if mods path exists
    let metadata = std::fs::metadata(mods_path)?;
    if !metadata.is_dir() {
        return Err(format!("Mods path is not a directory: {:?}", mods_path).into());
    }

    // Get all folders in mods directory
    let entries = std::fs::read_dir(mods_path)?;
    let folders: Vec<PathBuf> = entries
        .filter_map(|entry| {
            entry.ok().and_then(|e| {
                let path = e.path();
                if path.is_dir() {
                    Some(path)
                } else {
                    None
                }
            })
        })
        .collect();

    let folders_count = folders.len();
    if folders_count == 0 {
        return Ok(vec![]);
    }

    // Query mod IDs from each folder
    let mut mods: Vec<BaseMod> = Vec::new();

    for folder in folders {
        if let Some(mod_id) = query_mod_id(&folder)? {
            let folder_name = folder.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string());
            
            mods.push(BaseMod {
                mod_id: mod_id.clone(),
                mod_path: folder.to_string_lossy().to_string(),
                folder: folder_name,
                details: None,
                updated: None,
            });
        }
    }

    if mods.is_empty() {
        return Ok(vec![]);
    }

    // Query mods in batches of 50
    const BATCH_COUNT: usize = 50;

    // Create batches and query them in parallel (with small delays to avoid rate limiting)
    let mut batch_futures = Vec::new();
    let num_batches = (mods.len() + BATCH_COUNT - 1) / BATCH_COUNT;
    
    for batch_idx in 0..num_batches {
        let start = batch_idx * BATCH_COUNT;
        let end = std::cmp::min(start + BATCH_COUNT, mods.len());
        let mod_ids: Vec<String> = mods[start..end].iter().map(|m| m.mod_id.clone()).collect();
        let mod_indices = (start..end).collect::<Vec<usize>>();
        
        // Add delay between starting batches to avoid rate limiting
        if batch_idx > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(250 * batch_idx as u64)).await;
        }
        
        // Move mod_ids into the future to avoid lifetime issues
        let future = async move {
            query_mod_batch(&mod_ids, 0).await
        };
        batch_futures.push((future, mod_indices));
    }
    
    // Wait for all batches and update mods
    for (batch_future, mod_indices) in batch_futures {
        match batch_future.await {
            Ok(details) => {
                // Create HashMap for O(1) lookup instead of O(n) find()
                let details_map: std::collections::HashMap<String, WorkshopFileDetails> = details
                    .into_iter()
                    .map(|d| (d.publishedfileid.clone(), d))
                    .collect();
                
                // Update mods with details
                for idx in mod_indices {
                    if let Some(detail) = details_map.get(&mods[idx].mod_id) {
                        mods[idx].details = Some(detail.clone());
                    }
                }
            }
            Err(_e) => {
                // Failed to query batch, continue with next batch
            }
        }
    }

    let mods_with_details: Vec<&BaseMod> = mods.iter().filter(|m| m.details.is_some()).collect();
    
    if mods_with_details.is_empty() {
        return Ok(vec![]);
    }

    // Check which mods have updates available
    // First, filter mods that pass basic validation checks
    let mods_to_check: Vec<(usize, &BaseMod, &WorkshopFileDetails)> = mods.iter()
        .enumerate()
        .filter_map(|(idx, mod_ref)| {
            let details = mod_ref.details.as_ref()?;
            
            // Check for various error conditions
            if details.result == 9 {
                return None; // Mod has been removed/unlisted
            }
            if details.result != 1 {
                return None; // Invalid result code
            }
            if details.visibility != 0 {
                return None; // Private file
            }
            if details.banned {
                return None; // Banned file
            }
            if details.creator_app_id != 294100 {
                return None; // Not a Rimworld mod
            }
            if ignored_set.contains(&mod_ref.mod_id) {
                return None; // Ignored mod
            }
            
            Some((idx, mod_ref, details))
        })
        .collect();
    
    // Check last updated times in parallel using spawn_blocking
    // Also check if update is ignored via .ignoredupdate file
    let mut check_futures = Vec::new();
    for (_idx, mod_ref, details) in &mods_to_check {
        let mod_path = PathBuf::from(&mod_ref.mod_path);
        let time_updated = details.time_updated;
        let mod_id = mod_ref.mod_id.clone();
        
        let future = tokio::task::spawn_blocking(move || {
            // First check if update is ignored
            match get_ignored_update_timestamp(&mod_path) {
                Ok(Some(ignored_timestamp)) => {
                    // Update is ignored - check if remote timestamp is newer than ignored timestamp
                    let time_diff_seconds = (time_updated as i64 - ignored_timestamp) as f64;
                    // If remote is newer, mod needs update (ignore was for older version)
                    Some((mod_id, time_diff_seconds > 1.0))
                }
                Ok(None) => {
                    // No ignored update, check normally
                    let remote_date = std::time::UNIX_EPOCH + std::time::Duration::from_secs(time_updated as u64);
                    match get_mod_last_updated_time(&mod_path) {
                        Ok(last_updated_date) => {
                            let time_diff_seconds = remote_date.duration_since(last_updated_date)
                                .unwrap_or_default()
                                .as_secs() as f64;
                            Some((mod_id, time_diff_seconds > 1.0))
                        }
                        Err(_) => None,
                    }
                }
                Err(_) => {
                    // Error checking ignored update, fall back to normal check
                    let remote_date = std::time::UNIX_EPOCH + std::time::Duration::from_secs(time_updated as u64);
                    match get_mod_last_updated_time(&mod_path) {
                        Ok(last_updated_date) => {
                            let time_diff_seconds = remote_date.duration_since(last_updated_date)
                                .unwrap_or_default()
                                .as_secs() as f64;
                            Some((mod_id, time_diff_seconds > 1.0))
                        }
                        Err(_) => None,
                    }
                }
            }
        });
        check_futures.push(future);
    }
    
    // Wait for all checks and collect mods that need updates
    let mut mods_with_updates_map = std::collections::HashMap::new();
    let futures_results: Vec<_> = futures::future::join_all(check_futures).await;
    
    for (result, (_, mod_ref, _)) in futures_results.into_iter().zip(mods_to_check.iter()) {
        if let Ok(Some((mod_id, needs_update))) = result {
            if needs_update {
                if !mods_with_updates_map.contains_key(&mod_id) {
                    mods_with_updates_map.insert(mod_id, (*mod_ref).clone());
                }
            }
        }
    }
    
    let mods_with_updates: Vec<BaseMod> = mods_with_updates_map.into_values().collect();
    Ok(mods_with_updates)
}

/// List all installed mods quickly with only local data (no API calls)
/// This returns immediately with mod_id, folder, mod_path, and local metadata
pub fn list_installed_mods_fast(
    mods_path: &Path,
) -> Result<Vec<BaseMod>, Box<dyn std::error::Error>> {
    // Check if mods path exists
    let metadata = std::fs::metadata(mods_path)?;
    if !metadata.is_dir() {
        return Err(format!("Mods path is not a directory: {:?}", mods_path).into());
    }

    // Get all folders in mods directory
    let entries = std::fs::read_dir(mods_path)?;
    let folders: Vec<PathBuf> = entries
        .filter_map(|entry| {
            entry.ok().and_then(|e| {
                let path = e.path();
                if path.is_dir() {
                    Some(path)
                } else {
                    None
                }
            })
        })
        .collect();

    if folders.is_empty() {
        return Ok(vec![]);
    }

    // Query mod IDs from each folder and collect local data
    let mut mods: Vec<BaseMod> = Vec::new();

    for folder in folders {
        if let Some(mod_id) = query_mod_id(&folder)? {
            let folder_name = folder.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.to_string());
            
            // Get local metadata (folder size, last updated time)
            // Note: We don't populate details here - that will be done in background
            mods.push(BaseMod {
                mod_id: mod_id.clone(),
                mod_path: folder.to_string_lossy().to_string(),
                folder: folder_name,
                details: None, // Will be populated by update_mod_details later
                updated: None,
            });
        }
    }

    Ok(mods)
}

/// Update mod details from Steam API in background
/// This function fetches details for given mod IDs and returns updated BaseMod objects
pub async fn update_mod_details(
    mods: Vec<BaseMod>,
) -> Result<Vec<BaseMod>, Box<dyn std::error::Error>> {
    if mods.is_empty() {
        return Ok(vec![]);
    }

    // Query mods in batches of 50
    const BATCH_COUNT: usize = 50;
    let mut updated_mods = mods;

    // Create batches and query them in parallel (with small delays to avoid rate limiting)
    let mut batch_futures = Vec::new();
    let num_batches = (updated_mods.len() + BATCH_COUNT - 1) / BATCH_COUNT;
    
    for batch_idx in 0..num_batches {
        let start = batch_idx * BATCH_COUNT;
        let end = std::cmp::min(start + BATCH_COUNT, updated_mods.len());
        let mod_ids: Vec<String> = updated_mods[start..end].iter().map(|m| m.mod_id.clone()).collect();
        let mod_indices = (start..end).collect::<Vec<usize>>();
        
        // Add delay between starting batches to avoid rate limiting
        if batch_idx > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(250 * batch_idx as u64)).await;
        }
        
        // Move mod_ids into the future to avoid lifetime issues
        let future = async move {
            query_mod_batch(&mod_ids, 0).await
        };
        batch_futures.push((future, mod_indices));
    }
    
    // Wait for all batches and update mods
    for (batch_future, mod_indices) in batch_futures {
        match batch_future.await {
            Ok(details) => {
                // Create HashMap for O(1) lookup instead of O(n) find()
                let details_map: std::collections::HashMap<String, WorkshopFileDetails> = details
                    .into_iter()
                    .map(|d| (d.publishedfileid.clone(), d))
                    .collect();
                
                // Update mods with details
                for idx in mod_indices {
                    if let Some(detail) = details_map.get(&updated_mods[idx].mod_id) {
                        updated_mods[idx].details = Some(detail.clone());
                    }
                }
            }
            Err(_e) => {
                // Failed to query batch, continue with next batch
            }
        }
    }

    Ok(updated_mods)
}

/// List all installed mods in mods folder without checking for updates
/// This function now uses the fast version and returns immediately
pub async fn list_installed_mods(
    mods_path: &Path,
) -> Result<Vec<BaseMod>, Box<dyn std::error::Error>> {
    // Use fast version that returns immediately with local data only
    list_installed_mods_fast(mods_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_query_mod_id_valid() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        let about_path = mod_path.join("About");
        
        fs::create_dir_all(&about_path).unwrap();
        fs::write(about_path.join("PublishedFileId.txt"), "123456789").unwrap();
        
        let result = query_mod_id(&mod_path).unwrap();
        assert_eq!(result, Some("123456789".to_string()));
    }

    #[test]
    fn test_query_mod_id_no_about_folder() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("not_a_mod");
        
        let result = query_mod_id(&mod_path).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_query_mod_id_no_published_file_id() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("not_workshop_mod");
        let about_path = mod_path.join("About");
        
        fs::create_dir_all(&about_path).unwrap();
        
        let result = query_mod_id(&mod_path).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_get_mod_last_updated_time_with_lastupdated_file() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        let about_path = mod_path.join("About");
        
        fs::create_dir_all(&about_path).unwrap();
        let timestamp = 1609459200; // 2021-01-01 00:00:00 UTC
        fs::write(about_path.join(".lastupdated"), timestamp.to_string()).unwrap();
        
        let result = get_mod_last_updated_time(&mod_path).unwrap();
        let expected = std::time::UNIX_EPOCH + std::time::Duration::from_secs(timestamp as u64);
        
        // Allow small difference due to system time precision
        let diff = result.duration_since(expected).unwrap_or_default();
        assert!(diff.as_secs() < 2);
    }

    #[test]
    fn test_get_mod_last_updated_time_fallback_to_file_time() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("test_mod");
        let about_path = mod_path.join("About");
        
        fs::create_dir_all(&about_path).unwrap();
        fs::write(about_path.join("PublishedFileId.txt"), "123456789").unwrap();
        
        // Wait a bit to ensure file time is different
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        let result = get_mod_last_updated_time(&mod_path).unwrap();
        let now = std::time::SystemTime::now();
        
        // Should be recent (within last minute)
        let diff = now.duration_since(result).unwrap_or_default();
        assert!(diff.as_secs() < 60);
    }

    #[test]
    fn test_query_mod_id_empty_file() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("empty_mod");
        let about_path = mod_path.join("About");
        
        fs::create_dir_all(&about_path).unwrap();
        fs::write(about_path.join("PublishedFileId.txt"), "").unwrap();
        
        let result = query_mod_id(&mod_path).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_query_mod_id_whitespace_only() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("whitespace_mod");
        let about_path = mod_path.join("About");
        
        fs::create_dir_all(&about_path).unwrap();
        fs::write(about_path.join("PublishedFileId.txt"), "   \n\t  ").unwrap();
        
        let result = query_mod_id(&mod_path).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_query_mod_id_with_whitespace() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("whitespace_mod");
        let about_path = mod_path.join("About");
        
        fs::create_dir_all(&about_path).unwrap();
        fs::write(about_path.join("PublishedFileId.txt"), "  123456789  \n").unwrap();
        
        let result = query_mod_id(&mod_path).unwrap();
        assert_eq!(result, Some("123456789".to_string()));
    }

    #[test]
    fn test_get_mod_last_updated_time_invalid_timestamp() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("invalid_mod");
        let about_path = mod_path.join("About");
        
        fs::create_dir_all(&about_path).unwrap();
        fs::write(about_path.join(".lastupdated"), "invalid").unwrap();
        fs::write(about_path.join("PublishedFileId.txt"), "123456789").unwrap();
        
        // Should delete invalid file and fallback to file time
        let result = get_mod_last_updated_time(&mod_path).unwrap();
        let now = std::time::SystemTime::now();
        
        // Should be recent (within last minute)
        let diff = now.duration_since(result).unwrap_or_default();
        assert!(diff.as_secs() < 60);
        
        // Invalid file should be deleted
        assert!(!about_path.join(".lastupdated").exists());
    }

    #[test]
    fn test_get_mod_last_updated_time_negative_timestamp() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("negative_mod");
        let about_path = mod_path.join("About");
        
        fs::create_dir_all(&about_path).unwrap();
        fs::write(about_path.join(".lastupdated"), "-1000").unwrap();
        fs::write(about_path.join("PublishedFileId.txt"), "123456789").unwrap();
        
        // Should delete invalid file and fallback to file time
        let result = get_mod_last_updated_time(&mod_path).unwrap();
        let now = std::time::SystemTime::now();
        
        // Should be recent (within last minute)
        let diff = now.duration_since(result).unwrap_or_default();
        assert!(diff.as_secs() < 60);
    }

    #[test]
    fn test_get_mod_last_updated_time_zero_timestamp() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("zero_mod");
        let about_path = mod_path.join("About");
        
        fs::create_dir_all(&about_path).unwrap();
        fs::write(about_path.join(".lastupdated"), "0").unwrap();
        fs::write(about_path.join("PublishedFileId.txt"), "123456789").unwrap();
        
        // Should delete invalid file and fallback to file time
        let result = get_mod_last_updated_time(&mod_path).unwrap();
        let now = std::time::SystemTime::now();
        
        // Should be recent (within last minute)
        let diff = now.duration_since(result).unwrap_or_default();
        assert!(diff.as_secs() < 60);
    }

    #[test]
    fn test_get_mod_last_updated_time_fallback_to_mod_folder_time() {
        let temp_dir = TempDir::new().unwrap();
        let mod_path = temp_dir.path().join("no_about_mod");
        
        // Create mod folder but no About folder
        fs::create_dir_all(&mod_path).unwrap();
        
        // Wait a bit
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        let result = get_mod_last_updated_time(&mod_path).unwrap();
        let now = std::time::SystemTime::now();
        
        // Should be recent (within last minute)
        let diff = now.duration_since(result).unwrap_or_default();
        assert!(diff.as_secs() < 60);
    }
}

