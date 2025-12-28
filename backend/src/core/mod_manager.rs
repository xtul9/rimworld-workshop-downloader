use std::path::{Path, PathBuf};
use std::fs;
use crate::core::mod_scanner::query_mod_id;

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
                    
                    // Check if folder with this name already exists and has different mod ID
                    let proposed_path = mods_path.join(&folder_name);
                    if proposed_path.exists() && proposed_path.is_dir() {
                        if let Ok(Some(existing_mod_id)) = query_mod_id(&proposed_path) {
                            if existing_mod_id != mod_id {
                                // Folder exists with different mod ID, append modId to avoid conflict
                                folder_name = format!("{} ({})", folder_name, mod_id);
                                eprintln!("[ModUpdater] Folder \"{}\" exists with different mod ID, using \"{}\" instead", 
                                    Self::sanitize_folder_name(mod_title_to_use), folder_name);
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

        // Remove existing mod folder if it exists (async)
        if mod_destination_path.exists() {
            let path_to_remove = mod_destination_path.clone();
            tokio::task::spawn_blocking(move || {
                fs::remove_dir_all(&path_to_remove)
            }).await
            .map_err(|e| format!("Task panicked: {:?}", e))?
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

        eprintln!("[ModUpdater] Copying mod from {:?} to {:?}", source_path, mod_destination_path);
        copy_dir_all_async(&source_path, &mod_destination_path).await
            .map_err(|e| format!("Failed to copy mod: {}", e))?;

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

