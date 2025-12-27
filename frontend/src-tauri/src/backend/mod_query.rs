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

    // Query all batches sequentially to avoid lifetime issues
    for i in (0..mods.len()).step_by(BATCH_COUNT) {
        let batch_end = std::cmp::min(i + BATCH_COUNT, mods.len());
        let batch = &mods[i..batch_end];
        let mod_ids: Vec<String> = batch.iter().map(|m| m.mod_id.clone()).collect();
        
        match query_mod_batch(&mod_ids, 0).await {
            Ok(details) => {
                for detail in details {
                    if let Some(mod_ref) = mods[i..batch_end].iter_mut()
                        .find(|m| m.mod_id == detail.publishedfileid)
                    {
                        mod_ref.details = Some(detail);
                    }
                }
            }
            Err(_e) => {
                // Failed to query batch, continue with next batch
            }
        }
        
        // Delay between batches to avoid rate limiting
        if i + BATCH_COUNT < mods.len() {
            tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
        }
    }

    let mods_with_details: Vec<&BaseMod> = mods.iter().filter(|m| m.details.is_some()).collect();
    
    if mods_with_details.is_empty() {
        return Ok(vec![]);
    }

    // Check which mods have updates available
    let mut mods_with_updates_map = std::collections::HashMap::new();

    for mod_ref in &mods {
        let details = match &mod_ref.details {
            Some(d) => d,
            None => {
                continue;
            }
        };
        
        // Check for various error conditions
        if details.result == 9 {
            continue; // Mod has been removed/unlisted
        }

        if details.result != 1 {
            continue; // Invalid result code
        }

        if details.visibility != 0 {
            continue; // Private file
        }

        // Check if banned
        if details.banned {
            continue; // Banned file
        }

        if details.creator_app_id != 294100 {
            continue; // Not a Rimworld mod
        }

        // Compare dates
        let remote_date = std::time::UNIX_EPOCH + std::time::Duration::from_secs(details.time_updated as u64);
        let last_updated_date = get_mod_last_updated_time(Path::new(&mod_ref.mod_path))?;

        // Calculate time difference in seconds
        let time_diff_seconds = remote_date.duration_since(last_updated_date)
            .unwrap_or_default()
            .as_secs() as f64;
        
        // Consider mod as needing update if remote is at least 1 second newer
        let needs_update = time_diff_seconds > 1.0;

        // Skip if mod is in ignored list
        if ignored_mods.contains(&mod_ref.mod_id) {
            continue;
        }

        if needs_update {
            // Only add if we don't already have this modId (avoid duplicates)
            if !mods_with_updates_map.contains_key(&mod_ref.mod_id) {
                mods_with_updates_map.insert(mod_ref.mod_id.clone(), mod_ref.clone());
            }
        }
    }
    
    let mods_with_updates: Vec<BaseMod> = mods_with_updates_map.into_values().collect();
    Ok(mods_with_updates)
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

