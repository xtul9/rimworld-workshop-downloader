use std::path::{Path, PathBuf};
use std::fs;
use crate::core::mod_scanner::query_mod_id;
use crate::services::{ignore_path_in_watcher, WatcherIgnoreGuard, is_update_cancelled};
use quick_xml::events::Event;
use quick_xml::Reader;

/// Mod updater for copying mods from download folder to mods folder
pub struct ModUpdater;

impl ModUpdater {
    /// Sanitize folder name to be safe for filesystem
    pub fn sanitize_folder_name(name: &str) -> String {
        let mut sanitized: String = name
            .chars()
            .filter(|c| !matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' | '\x00'..='\x1F'))
            .collect();
        
        // Replace multiple whitespace with single space
        sanitized = sanitized
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ");
        
        sanitized = sanitized.trim().to_string();
        
        // Remove leading/trailing dots and spaces
        sanitized = sanitized.trim_matches(|c: char| c == '.' || c.is_whitespace()).to_string();
        
        // If empty after sanitization, use a fallback
        if sanitized.is_empty() {
            return "Mod".to_string();
        }
        
        // Limit length to avoid filesystem issues
        if sanitized.len() > 200 {
            sanitized.truncate(200);
            sanitized = sanitized.trim().to_string();
        }
        
        sanitized
    }

    /// Update/Copy mod from download folder to mods folder
    pub async fn update_mod(
        &self,
        mod_id: &str,
        mod_path: &Path,
        download_path: &Path,
        mods_path: &Path,
        existing_folder_name: Option<&str>,
        create_backup: bool,
        backup_directory: Option<&Path>,
        mod_title: Option<&str>,
        force_overwrite_corrupted: Option<bool>,
    ) -> Result<PathBuf, String> {
        // Use existing folder name if provided, otherwise find existing folder with same mod ID, otherwise use mod title
        let folder_name = if let Some(name) = existing_folder_name {
            name.to_string()
        } else {
            match self.find_existing_mod_folder(mods_path, mod_id).await
                .map_err(|e| format!("Failed to find existing mod folder: {}", e))? {
                Some(existing_folder) => {
                    existing_folder.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(mod_id)
                        .to_string()
                }
                None => {
                    // Use mod title if available, otherwise fall back to modId
                    let mod_title_to_use = mod_title.unwrap_or(mod_id);
                    let mut folder_name = Self::sanitize_folder_name(mod_title_to_use);
                    
                    // Get packageId from the source mod (the one being downloaded)
                    let source_package_id = if mod_path.exists() && mod_path.is_dir() {
                        Self::get_package_id(mod_path)
                    } else {
                        let fallback_path = download_path.join(mod_id);
                        if fallback_path.exists() && fallback_path.is_dir() {
                            Self::get_package_id(&fallback_path)
                        } else {
                            None
                        }
                    };
                    
                    // Check if folder with this name already exists
                    let mut proposed_path = mods_path.join(&folder_name);
                    if proposed_path.exists() && proposed_path.is_dir() {
                        // Check if existing mod is corrupted
                        if Self::is_mod_corrupted(&proposed_path) {
                            // If force_overwrite_corrupted is Some(true), overwrite
                            // If force_overwrite_corrupted is Some(false), rename
                            // If force_overwrite_corrupted is None, return error to ask user
                            match force_overwrite_corrupted {
                                Some(true) => {
                                    // Force overwrite - continue with same folder name
                                    eprintln!("[ModUpdater] Force overwriting corrupted mod at {:?}", proposed_path);
                                }
                                Some(false) => {
                                    // Force rename - change folder name
                                    let base_name = folder_name.clone();
                                    loop {
                                        folder_name = format!("{}_", folder_name);
                                        proposed_path = mods_path.join(&folder_name);
                                        if !proposed_path.exists() {
                                            break;
                                        }
                                        // Safety limit
                                        if folder_name.len() > base_name.len() + 50 {
                                            folder_name = format!("{}_{}", base_name, mod_id);
                                            break;
                                        }
                                    }
                                    eprintln!("[ModUpdater] Force renaming corrupted mod, using \"{}\" instead", folder_name);
                                }
                                None => {
                                    // Mod is corrupted - return special error to ask user for decision
                                    return Err(format!("CORRUPTED_MOD_CONFLICT:{}:{}", folder_name, mod_id));
                                }
                            }
                        }
                        
                        // Only check package ID if path exists and is not corrupted
                        // (when force_overwrite_corrupted is Some(false), proposed_path may not exist)
                        if proposed_path.exists() && !Self::is_mod_corrupted(&proposed_path) {
                            let existing_package_id = Self::get_package_id(&proposed_path);
                        
                        match (source_package_id.as_ref(), existing_package_id.as_ref()) {
                            // Both have packageId - compare them
                            (Some(src_id), Some(existing_id)) => {
                                if src_id != existing_id {
                                    // Different packageId - change folder name to avoid conflict
                                    // Try adding "_" until we find a unique name or one with same packageId
                                    let base_name = folder_name.clone();
                                    loop {
                                        folder_name = format!("{}_", folder_name);
                                        proposed_path = mods_path.join(&folder_name);
                                        if !proposed_path.exists() {
                                            break;
                                        }
                                        // Check if existing folder has same packageId
                                        if let Some(existing_id_check) = Self::get_package_id(&proposed_path) {
                                            if existing_id_check == *src_id {
                                                // Same packageId - can overwrite
                                                break;
                                            }
                                        }
                                        // Safety limit - if we've added too many underscores, use mod_id
                                        if folder_name.len() > base_name.len() + 50 {
                                            folder_name = format!("{}_{}", base_name, mod_id);
                                            break;
                                        }
                                    }
                                    eprintln!("[ModUpdater] Folder \"{}\" exists with different packageId ({} vs {}), using \"{}\" instead", 
                                        Self::sanitize_folder_name(mod_title_to_use), existing_id, src_id, folder_name);
                                }
                                // Same packageId - will overwrite (no change to folder_name)
                            }
                            // Source has packageId, existing doesn't - different mods, change name
                            (Some(_), None) => {
                                let base_name = folder_name.clone();
                                loop {
                                    folder_name = format!("{}_", folder_name);
                                    proposed_path = mods_path.join(&folder_name);
                                    if !proposed_path.exists() {
                                        break;
                                    }
                                    // Safety limit
                                    if folder_name.len() > base_name.len() + 50 {
                                        folder_name = format!("{}_{}", base_name, mod_id);
                                        break;
                                    }
                                }
                                eprintln!("[ModUpdater] Folder \"{}\" exists but has no packageId, using \"{}\" instead", 
                                    Self::sanitize_folder_name(mod_title_to_use), folder_name);
                            }
                            // Source doesn't have packageId, existing does - different mods, change name
                            (None, Some(_)) => {
                                let base_name = folder_name.clone();
                                loop {
                                    folder_name = format!("{}_", folder_name);
                                    proposed_path = mods_path.join(&folder_name);
                                    if !proposed_path.exists() {
                                        break;
                                    }
                                    // Safety limit
                                    if folder_name.len() > base_name.len() + 50 {
                                        folder_name = format!("{}_{}", base_name, mod_id);
                                        break;
                                    }
                                }
                                eprintln!("[ModUpdater] Folder \"{}\" exists with packageId but source doesn't, using \"{}\" instead", 
                                    Self::sanitize_folder_name(mod_title_to_use), folder_name);
                            }
                            // Neither has packageId - fall back to mod_id check
                            (None, None) => {
                                if let Ok(Some(existing_mod_id)) = query_mod_id(&proposed_path) {
                                    if existing_mod_id != mod_id {
                                        // Folder exists with different mod ID, append modId to avoid conflict
                                        folder_name = format!("{} ({})", folder_name, mod_id);
                                        eprintln!("[ModUpdater] Folder \"{}\" exists with different mod ID, using \"{}\" instead", 
                                            Self::sanitize_folder_name(mod_title_to_use), folder_name);
                                    }
                                }
                            }
                        }
                        }
                    }
                    
                    eprintln!("[ModUpdater] No existing folder found for mod {}, will use \"{}\" as folder name", mod_id, folder_name);
                    folder_name
                }
            }
        };
        
        let mod_destination_path = mods_path.join(&folder_name);

        // Ensure mods folder exists
        fs::create_dir_all(mods_path)
            .map_err(|e| format!("Failed to create mods directory: {}", e))?;

        // Create backup if requested
        if create_backup {
            // Check if update was cancelled before starting backup
            if is_update_cancelled() {
                return Err("Update cancelled by user".to_string());
            }
            
            if let Some(backup_dir) = backup_directory {
                fs::create_dir_all(backup_dir)
                    .map_err(|e| format!("Failed to create backup directory: {}", e))?;
                let backup_path = backup_dir.join(&folder_name);
                
                // Remove old backup if exists
                if backup_path.exists() {
                    fs::remove_dir_all(&backup_path)
                        .map_err(|e| format!("Failed to remove old backup: {}", e))?;
                }
                
                // Copy current mod to backup directory
                if mod_destination_path.exists() {
                    copy_dir_all_async(&mod_destination_path, &backup_path).await
                        .map_err(|e| format!("Failed to create backup: {}", e))?;
                    eprintln!("[ModUpdater] Created backup for mod {} at {:?}", mod_id, backup_path);
                }
            }
        }

        // Ignore this path in mod watcher during update operation
        ignore_path_in_watcher(mod_destination_path.clone()).await;
        let _guard = WatcherIgnoreGuard::new(mod_destination_path.clone()).await;

        // Give mod watcher a moment to close any open file handles
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Remove existing mod folder if it exists (async with retry)
        // Use retry logic to handle cases where mod watcher or other processes have files open
        if mod_destination_path.exists() {
            // Check if update was cancelled before removing existing folder
            if is_update_cancelled() {
                return Err("Update cancelled by user".to_string());
            }
            
            Self::remove_dir_with_retry(&mod_destination_path, 3, 200).await
                .map_err(|e| format!("Failed to remove existing mod folder: {}", e))?;
        }

        // Copy mod from download folder to game mods folder
        let source_path = if mod_path.exists() && mod_path.is_dir() {
            eprintln!("[ModUpdater] Using mod_path as source: {:?}", mod_path);
            mod_path.to_path_buf()
        } else {
            let fallback_path = download_path.join(mod_id);
            eprintln!("[ModUpdater] mod_path {:?} doesn't exist, using fallback: {:?}", mod_path, fallback_path);
            fallback_path
        };
        
        if !source_path.exists() || !source_path.is_dir() {
            return Err(format!("Source mod folder not found: {:?}", source_path));
        }

        // Verify source mod is complete before copying
        if !Self::verify_mod_complete(&source_path) {
            return Err(format!("Source mod at {:?} appears incomplete or invalid. Refusing to copy.", source_path));
        }

        // Check if update was cancelled before starting copy operation
        if is_update_cancelled() {
            return Err("Update cancelled by user".to_string());
        }

        eprintln!("[ModUpdater] Copying mod from {:?} to {:?}", source_path, mod_destination_path);
        copy_dir_all_async(&source_path, &mod_destination_path).await
            .map_err(|e| format!("Failed to copy mod: {}", e))?;

        // Verify copied mod is complete
        if !Self::verify_mod_complete(&mod_destination_path) {
            return Err(format!("Copied mod at {:?} appears incomplete. Copy may have failed.", mod_destination_path));
        }

        // Ensure PublishedFileId.txt exists after copying
        Self::ensure_published_file_id(&mod_destination_path, mod_id).await
            .map_err(|e| format!("Failed to create PublishedFileId.txt: {}", e))?;

        // Manually unignore the path (this consumes the guard and prevents Drop from running)
        // If we reach here, the operation was successful
        _guard.unignore().await;

        eprintln!("[ModUpdater] Mod {} copied successfully to {:?}", mod_id, mod_destination_path);

        Ok(mod_destination_path)
    }

    /// Find existing mod folder with the given mod ID
    async fn find_existing_mod_folder(&self, mods_path: &Path, mod_id: &str) -> Result<Option<PathBuf>, String> {
        let entries = fs::read_dir(mods_path)
            .map_err(|e| format!("Failed to read mods directory: {}", e))?;
        
        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();
            
            if path.is_dir() {
                if let Ok(Some(found_mod_id)) = query_mod_id(&path) {
                    if found_mod_id == mod_id {
                        return Ok(Some(path));
                    }
                }
            }
        }
        
        Ok(None)
    }

    /// Ensure PublishedFileId.txt exists in the mod's About folder
    /// Creates the file if it doesn't exist
    async fn ensure_published_file_id(mod_path: &Path, mod_id: &str) -> Result<(), String> {
        let about_path = mod_path.join("About");
        
        // Check if About folder exists, if not, create it
        if !about_path.exists() {
            fs::create_dir_all(&about_path)
                .map_err(|e| format!("Failed to create About directory: {}", e))?;
            eprintln!("[ModUpdater] Created About directory at {:?}", about_path);
        }
        
        let file_id_path = about_path.join("PublishedFileId.txt");
        
        // Check if PublishedFileId.txt already exists
        if file_id_path.exists() {
            // Verify it contains the correct mod ID
            match fs::read_to_string(&file_id_path) {
                Ok(content) => {
                    let existing_id = content.trim();
                    if existing_id == mod_id {
                        // File exists and has correct ID, nothing to do
                        return Ok(());
                    } else {
                        eprintln!("[ModUpdater] PublishedFileId.txt exists but has different ID ({} vs {}), updating it", existing_id, mod_id);
                    }
                }
                Err(e) => {
                    eprintln!("[ModUpdater] Failed to read existing PublishedFileId.txt: {}, will recreate it", e);
                }
            }
        }
        
        // Create or update PublishedFileId.txt
        tokio::task::spawn_blocking({
            let file_id_path = file_id_path.clone();
            let mod_id = mod_id.to_string();
            move || {
                fs::write(&file_id_path, mod_id)
                    .map_err(|e| format!("Failed to write PublishedFileId.txt: {}", e))
            }
        }).await
        .map_err(|e| format!("Task panicked: {:?}", e))?
        .map_err(|e| e)?;
        
        eprintln!("[ModUpdater] Created/updated PublishedFileId.txt at {:?} with ID {}", file_id_path, mod_id);
        Ok(())
    }

    /// Extract packageId from About.xml
    /// Returns None if About.xml doesn't exist or packageId cannot be found
    fn get_package_id(mod_path: &Path) -> Option<String> {
        let about_path = mod_path.join("About");
        let about_xml_path = about_path.join("About.xml");
        
        if !about_xml_path.exists() {
            return None;
        }
        
        let content = match fs::read_to_string(&about_xml_path) {
            Ok(c) => c,
            Err(_) => return None,
        };
        
        let mut reader = Reader::from_str(&content);
        reader.trim_text(true);
        
        let mut in_mod_metadata = false;
        let mut depth = 0; // Track nesting depth within ModMetaData
        let mut in_package_id = false;
        let mut package_id = String::new();
        
        loop {
            match reader.read_event() {
                Ok(Event::Start(e)) => {
                    let name = e.name();
                    if name.as_ref() == b"ModMetaData" {
                        in_mod_metadata = true;
                        depth = 0; // Reset depth when entering ModMetaData
                    } else if in_mod_metadata {
                        depth += 1;
                        // Only accept packageId if we're directly under ModMetaData (depth == 1)
                        if depth == 1 && name.as_ref() == b"packageId" {
                            in_package_id = true;
                        }
                    }
                }
                Ok(Event::Text(e)) => {
                    // Only capture text if we're in packageId and directly under ModMetaData
                    if in_package_id && depth == 1 {
                        package_id = e.unescape().unwrap_or_default().to_string();
                    }
                }
                Ok(Event::End(e)) => {
                    let name = e.name();
                    if name.as_ref() == b"packageId" {
                        // Check depth before decreasing it
                        let is_direct_child = in_package_id && depth == 1;
                        if in_mod_metadata {
                            depth -= 1;
                        }
                        if is_direct_child {
                            in_package_id = false;
                            if !package_id.is_empty() {
                                return Some(package_id.trim().to_string());
                            }
                        }
                    } else if name.as_ref() == b"ModMetaData" {
                        in_mod_metadata = false;
                        depth = 0;
                    } else if in_mod_metadata {
                        depth -= 1;
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    eprintln!("[ModUpdater] Error parsing About.xml: {:?}", e);
                    return None;
                }
                _ => {}
            }
        }
        
        None
    }

    /// Remove directory with retry logic and delay to handle file locks
    /// This is useful when mod watcher or other processes might have files open
    async fn remove_dir_with_retry(path: &Path, max_retries: u32, delay_ms: u64) -> Result<(), String> {
        let path = path.to_path_buf();
        
        for attempt in 1..=max_retries {
            let result = tokio::task::spawn_blocking({
                let path = path.clone();
                move || {
                    if path.exists() {
                        fs::remove_dir_all(&path)
                    } else {
                        Ok(())
                    }
                }
            }).await
            .map_err(|e| format!("Task panicked: {:?}", e))?;
            
            match result {
                Ok(()) => {
                    if attempt > 1 {
                        eprintln!("[ModUpdater] Successfully removed directory after {} attempt(s): {:?}", attempt, path);
                    }
                    return Ok(());
                }
                Err(e) => {
                    if attempt < max_retries {
                        eprintln!("[ModUpdater] Attempt {} failed to remove directory {:?}: {}. Retrying in {}ms...", 
                            attempt, path, e, delay_ms);
                        tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                    } else {
                        return Err(format!("Failed to remove directory after {} attempts: {}", max_retries, e));
                    }
                }
            }
        }
        
        Err(format!("Failed to remove directory after {} attempts", max_retries))
    }

    /// Verify that a mod is complete before copying
    fn verify_mod_complete(mod_path: &Path) -> bool {
        // Check if mod folder exists and is a directory
        if !mod_path.exists() || !mod_path.is_dir() {
            eprintln!("[ModUpdater] Mod path does not exist or is not a directory: {:?}", mod_path);
            return false;
        }
        
        // Check if folder has any content
        let has_content = if let Ok(entries) = fs::read_dir(mod_path) {
            entries.take(1).count() > 0
        } else {
            false
        };
        
        if !has_content {
            eprintln!("[ModUpdater] Mod folder is empty: {:?}", mod_path);
            return false;
        }
        
        // Check for About folder (essential for RimWorld mods)
        let about_path = mod_path.join("About");
        if !about_path.exists() || !about_path.is_dir() {
            eprintln!("[ModUpdater] Mod missing About folder: {:?}", mod_path);
            return false;
        }
        
        true
    }

    /// Check if a mod is corrupted (missing About folder or About.xml)
    /// Returns true if mod is corrupted, false otherwise
    pub fn is_mod_corrupted(mod_path: &Path) -> bool {
        // Check if mod folder exists and is a directory
        if !mod_path.exists() || !mod_path.is_dir() {
            return false; // Not a mod folder at all
        }
        
        // Check for About folder
        let about_path = mod_path.join("About");
        if !about_path.exists() || !about_path.is_dir() {
            return true; // Missing About folder - corrupted
        }
        
        // Check for About.xml
        let about_xml_path = about_path.join("About.xml");
        if !about_xml_path.exists() {
            return true; // Missing About.xml - corrupted
        }
        
        false // Mod appears to be valid
    }
}

/// Recursively copy directory (async version using spawn_blocking)
pub async fn copy_dir_all_async(src: &Path, dst: &Path) -> Result<(), String> {
    let src = src.to_path_buf();
    let dst = dst.to_path_buf();
    
    tokio::task::spawn_blocking(move || {
        copy_dir_all_sync(&src, &dst)
    }).await
    .map_err(|e| format!("Task panicked: {:?}", e))?
}

/// Recursively copy directory (synchronous version for use in spawn_blocking)
fn copy_dir_all_sync(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst)
        .map_err(|e| format!("Failed to create directory {}: {}", dst.display(), e))?;
    
    for entry in fs::read_dir(src)
        .map_err(|e| format!("Failed to read directory {}: {}", src.display(), e))? {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();
        let file_name = entry.file_name();
        let dst_path = dst.join(&file_name);
        
        if path.is_dir() {
            copy_dir_all_sync(&path, &dst_path)?;
        } else {
            fs::copy(&path, &dst_path)
                .map_err(|e| format!("Failed to copy {} to {}: {}", path.display(), dst_path.display(), e))?;
        }
    }
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_sanitize_folder_name() {
        assert_eq!(ModUpdater::sanitize_folder_name("Test Mod"), "Test Mod");
        assert_eq!(ModUpdater::sanitize_folder_name("Test<Mod>"), "TestMod");
        assert_eq!(ModUpdater::sanitize_folder_name(""), "Mod");
        assert_eq!(ModUpdater::sanitize_folder_name("   "), "Mod");
    }

    #[tokio::test]
    async fn test_find_existing_mod_folder() {
        let temp_dir = TempDir::new().unwrap();
        let mods_path = temp_dir.path();
        
        // Create a mod folder
        let mod_folder = mods_path.join("existing_mod");
        let about_path = mod_folder.join("About");
        fs::create_dir_all(&about_path).unwrap();
        fs::write(about_path.join("PublishedFileId.txt"), "123456789").unwrap();
        
        let updater = ModUpdater;
        let result = updater.find_existing_mod_folder(mods_path, "123456789").await.unwrap();
        
        assert!(result.is_some());
        assert_eq!(result.unwrap(), mod_folder);
    }

    #[tokio::test]
    async fn test_find_existing_mod_folder_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let mods_path = temp_dir.path();
        
        // Create a mod folder with different ID
        let mod_folder = mods_path.join("other_mod");
        let about_path = mod_folder.join("About");
        fs::create_dir_all(&about_path).unwrap();
        fs::write(about_path.join("PublishedFileId.txt"), "999999999").unwrap();
        
        let updater = ModUpdater;
        let result = updater.find_existing_mod_folder(mods_path, "123456789").await.unwrap();
        
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_find_existing_mod_folder_multiple_mods() {
        let temp_dir = TempDir::new().unwrap();
        let mods_path = temp_dir.path();
        
        // Create multiple mod folders
        let mod_folder1 = mods_path.join("mod1");
        let about_path1 = mod_folder1.join("About");
        fs::create_dir_all(&about_path1).unwrap();
        fs::write(about_path1.join("PublishedFileId.txt"), "111111111").unwrap();
        
        let mod_folder2 = mods_path.join("mod2");
        let about_path2 = mod_folder2.join("About");
        fs::create_dir_all(&about_path2).unwrap();
        fs::write(about_path2.join("PublishedFileId.txt"), "222222222").unwrap();
        
        let updater = ModUpdater;
        let result1 = updater.find_existing_mod_folder(mods_path, "111111111").await.unwrap();
        let result2 = updater.find_existing_mod_folder(mods_path, "222222222").await.unwrap();
        
        assert!(result1.is_some());
        assert_eq!(result1.unwrap(), mod_folder1);
        assert!(result2.is_some());
        assert_eq!(result2.unwrap(), mod_folder2);
    }

    #[test]
    fn test_sanitize_folder_name_normal() {
        assert_eq!(ModUpdater::sanitize_folder_name("Test Mod"), "Test Mod");
        assert_eq!(ModUpdater::sanitize_folder_name("My Awesome Mod"), "My Awesome Mod");
    }

    #[test]
    fn test_sanitize_folder_name_invalid_chars() {
        assert_eq!(ModUpdater::sanitize_folder_name("Test<Mod>"), "TestMod");
        assert_eq!(ModUpdater::sanitize_folder_name("Test:Mod"), "TestMod");
        assert_eq!(ModUpdater::sanitize_folder_name("Test/Mod"), "TestMod");
        assert_eq!(ModUpdater::sanitize_folder_name("Test\\Mod"), "TestMod");
        assert_eq!(ModUpdater::sanitize_folder_name("Test|Mod"), "TestMod");
        assert_eq!(ModUpdater::sanitize_folder_name("Test?Mod"), "TestMod");
        assert_eq!(ModUpdater::sanitize_folder_name("Test*Mod"), "TestMod");
    }

    #[test]
    fn test_sanitize_folder_name_control_chars() {
        assert_eq!(ModUpdater::sanitize_folder_name("Test\x00Mod"), "TestMod");
        assert_eq!(ModUpdater::sanitize_folder_name("Test\nMod"), "TestMod");
        assert_eq!(ModUpdater::sanitize_folder_name("Test\tMod"), "TestMod");
    }

    #[test]
    fn test_sanitize_folder_name_multiple_spaces() {
        assert_eq!(ModUpdater::sanitize_folder_name("Test    Mod"), "Test Mod");
        assert_eq!(ModUpdater::sanitize_folder_name("Test   Mod   Name"), "Test Mod Name");
    }

    #[test]
    fn test_sanitize_folder_name_leading_trailing_dots() {
        assert_eq!(ModUpdater::sanitize_folder_name("...Test Mod..."), "Test Mod");
        assert_eq!(ModUpdater::sanitize_folder_name(".Test Mod."), "Test Mod");
    }

    #[test]
    fn test_sanitize_folder_name_leading_trailing_spaces() {
        assert_eq!(ModUpdater::sanitize_folder_name("   Test Mod   "), "Test Mod");
        assert_eq!(ModUpdater::sanitize_folder_name(" Test Mod "), "Test Mod");
    }

    #[test]
    fn test_sanitize_folder_name_empty() {
        assert_eq!(ModUpdater::sanitize_folder_name(""), "Mod");
        assert_eq!(ModUpdater::sanitize_folder_name("   "), "Mod");
        assert_eq!(ModUpdater::sanitize_folder_name("..."), "Mod");
    }

    #[test]
    fn test_sanitize_folder_name_long() {
        let long_name = "A".repeat(300);
        let result = ModUpdater::sanitize_folder_name(&long_name);
        assert!(result.len() <= 200);
        assert!(!result.is_empty());
    }

    #[tokio::test]
    async fn test_update_mod_basic() {
        let temp_dir = TempDir::new().unwrap();
        let mods_path = temp_dir.path().join("mods");
        let download_path = temp_dir.path().join("download");
        
        // Create source mod
        let source_mod = download_path.join("123456789");
        let source_about = source_mod.join("About");
        fs::create_dir_all(&source_about).unwrap();
        fs::write(source_about.join("PublishedFileId.txt"), "123456789").unwrap();
        fs::write(source_mod.join("test.txt"), "test content").unwrap();
        
        let updater = ModUpdater;
        let result = updater.update_mod(
            "123456789",
            &source_mod,
            &download_path,
            &mods_path,
            Some("123456789"), // Provide folder name explicitly
            false,
            None,
            None,
            None, // force_overwrite_corrupted
        ).await.unwrap();
        
        assert!(result.exists());
        assert!(result.join("test.txt").exists());
        assert_eq!(result.file_name().unwrap(), "123456789");
    }

    #[tokio::test]
    async fn test_update_mod_with_existing_folder_name() {
        let temp_dir = TempDir::new().unwrap();
        let mods_path = temp_dir.path().join("mods");
        let download_path = temp_dir.path().join("download");
        
        // Create source mod
        let source_mod = download_path.join("123456789");
        let source_about = source_mod.join("About");
        fs::create_dir_all(&source_about).unwrap();
        fs::write(source_about.join("PublishedFileId.txt"), "123456789").unwrap();
        fs::write(source_mod.join("test.txt"), "test content").unwrap();
        
        let updater = ModUpdater;
        let result = updater.update_mod(
            "123456789",
            &source_mod,
            &download_path,
            &mods_path,
            Some("My Custom Mod Name"),
            false,
            None,
            None,
            None, // force_overwrite_corrupted
        ).await.unwrap();
        
        assert!(result.exists());
        assert_eq!(result.file_name().unwrap(), "My Custom Mod Name");
    }

    #[tokio::test]
    async fn test_update_mod_with_backup() {
        let temp_dir = TempDir::new().unwrap();
        let mods_path = temp_dir.path().join("mods");
        let download_path = temp_dir.path().join("download");
        let backup_dir = temp_dir.path().join("backup");
        
        // Create existing mod
        let existing_mod = mods_path.join("123456789");
        let existing_about = existing_mod.join("About");
        fs::create_dir_all(&existing_about).unwrap();
        fs::write(existing_about.join("PublishedFileId.txt"), "123456789").unwrap();
        fs::write(existing_mod.join("old.txt"), "old content").unwrap();
        
        // Create source mod
        let source_mod = download_path.join("123456789");
        let source_about = source_mod.join("About");
        fs::create_dir_all(&source_about).unwrap();
        fs::write(source_about.join("PublishedFileId.txt"), "123456789").unwrap();
        fs::write(source_mod.join("new.txt"), "new content").unwrap();
        
        let updater = ModUpdater;
        let result = updater.update_mod(
            "123456789",
            &source_mod,
            &download_path,
            &mods_path,
            Some("123456789"),
            true,
            Some(&backup_dir),
            None,
            None, // force_overwrite_corrupted
        ).await.unwrap();
        
        assert!(result.exists());
        assert!(result.join("new.txt").exists());
        assert!(!result.join("old.txt").exists());
        
        // Check backup exists
        let backup_path = backup_dir.join("123456789");
        assert!(backup_path.exists());
        assert!(backup_path.join("old.txt").exists());
    }
}

