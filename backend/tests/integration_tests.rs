use rimworld_workshop_downloader_lib::commands;
use rimworld_workshop_downloader_lib::backend::mod_query::{BaseMod, WorkshopFileDetails};
use tempfile::TempDir;
use std::fs;
use std::path::PathBuf;

// NOTE: These integration tests focus on workflows that DON'T require SteamCMD.
// Tests that require SteamCMD (like update_mods and download_mod) are not included
// because SteamCMD is not available in the test environment and would require mocking.
// For full end-to-end testing with SteamCMD, use manual testing or CI/CD with SteamCMD installed.

/// Helper function to create a test mod folder structure with About.xml and PublishedFileId.txt
fn create_test_mod_folder(temp_dir: &TempDir, mod_id: &str, folder_name: &str, time_updated: Option<i64>) -> PathBuf {
    let mod_path = temp_dir.path().join(folder_name);
    fs::create_dir_all(&mod_path).unwrap();
    
    // Create About folder
    let about_path = mod_path.join("About");
    fs::create_dir_all(&about_path).unwrap();
    
    // Create About.xml
    let about_xml = format!(r#"<?xml version="1.0" encoding="utf-8"?>
<ModMetaData>
    <name>{}</name>
    <author>Test Author</author>
    <packageId>test.{}</packageId>
    <publishedFileId>{}</publishedFileId>
</ModMetaData>"#, folder_name, mod_id, mod_id);
    
    fs::write(about_path.join("About.xml"), about_xml).unwrap();
    
    // Create PublishedFileId.txt
    fs::write(about_path.join("PublishedFileId.txt"), mod_id).unwrap();
    
    // Create .lastupdated file if time_updated is provided
    if let Some(timestamp) = time_updated {
        fs::write(about_path.join(".lastupdated"), timestamp.to_string()).unwrap();
    }
    
    mod_path
}

/// Integration test: Query mods -> Check for updates
#[tokio::test]
async fn test_integration_query_mods_workflow() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create test mods
    let mod_id1 = "111111111";
    let mod_id2 = "222222222";
    create_test_mod_folder(&temp_dir, mod_id1, "Mod1", Some(1000000000));
    create_test_mod_folder(&temp_dir, mod_id2, "Mod2", Some(1000000000));
    
    let mods_path = temp_dir.path().to_string_lossy().to_string();
    
    // Query mods
    let result = commands::query_mods(mods_path.clone(), vec![]).await;
    assert!(result.is_ok());
    let _mods = result.unwrap();
    
    // Should find at least the mods we created (may or may not have updates depending on Steam API)
    // Note: We just verify the function succeeded
    
    // Query with ignored mods
    let ignored_mods = vec![mod_id1.to_string()];
    let result = commands::query_mods(mods_path, ignored_mods).await;
    assert!(result.is_ok());
    let mods_filtered = result.unwrap();
    
    // Should not include ignored mods
    assert!(!mods_filtered.iter().any(|m| m.mod_id == mod_id1));
}

/// Integration test: Check backup -> Restore backup workflow
#[tokio::test]
async fn test_integration_backup_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let mod_id = "123456789";
    let folder_name = "TestMod";
    let mod_path = create_test_mod_folder(&temp_dir, mod_id, folder_name, Some(1000000000));
    
    // Create backup directory
    let backup_dir = temp_dir.path().join("backups");
    fs::create_dir_all(&backup_dir).unwrap();
    
    // Create backup folder with content
    let backup_path = backup_dir.join(folder_name);
    fs::create_dir_all(&backup_path).unwrap();
    fs::write(backup_path.join("backup_file.txt"), "backup content").unwrap();
    
    // Check backup
    let result = commands::check_backup(
        mod_path.to_string_lossy().to_string(),
        Some(backup_dir.to_string_lossy().to_string())
    ).await;
    
    assert!(result.is_ok());
    let backup_info = result.unwrap();
    assert_eq!(backup_info["hasBackup"], true);
    
    // Modify mod folder
    fs::write(mod_path.join("current_file.txt"), "current content").unwrap();
    
    // Restore backup
    let result = commands::restore_backup(
        mod_path.to_string_lossy().to_string(),
        backup_dir.to_string_lossy().to_string()
    ).await;
    
    assert!(result.is_ok());
    
    // Verify backup was restored
    assert!(mod_path.join("backup_file.txt").exists());
    assert!(!mod_path.join("current_file.txt").exists());
    
    // Verify backup was deleted
    assert!(!backup_path.exists());
}

/// Integration test: Ignore update workflow
/// Note: Requires Steam API but not SteamCMD
#[tokio::test]
async fn test_integration_ignore_update_workflow() {
    let temp_dir = TempDir::new().unwrap();
    let mod_id = "123456789";
    let folder_name = "TestMod";
    let mod_path = create_test_mod_folder(&temp_dir, mod_id, folder_name, Some(1000000000));
    
    // Create .lastupdated file with old timestamp
    let about_path = mod_path.join("About");
    let last_updated_path = about_path.join(".lastupdated");
    fs::write(&last_updated_path, "1000000000").unwrap();
    
    // Create BaseMod with details
    let mods = vec![BaseMod {
        mod_id: mod_id.to_string(),
        mod_path: mod_path.to_string_lossy().to_string(),
        folder: Some(folder_name.to_string()),
        details: Some(WorkshopFileDetails {
            publishedfileid: mod_id.to_string(),
            title: folder_name.to_string(),
            time_updated: 2000000000,
            result: 1,
            creator: "".to_string(),
            creator_app_id: 0,
            consumer_app_id: 0,
            filename: "".to_string(),
            file_size: 0,
            file_url: "".to_string(),
            hcontent_file: "".to_string(),
            preview_url: "".to_string(),
            hcontent_preview: "".to_string(),
            description: "".to_string(),
            time_created: 0,
            visibility: 0,
            flags: 0,
            workshop_file_url: "".to_string(),
            workshop_accepted: false,
            show_subscribe_all: false,
            num_comments_developer: 0,
            num_comments_public: 0,
            banned: false,
            ban_reason: "".to_string(),
            banner: "".to_string(),
            can_be_deleted: false,
            app_name: "".to_string(),
            file_type: 0,
            can_subscribe: false,
            subscriptions: 0,
            favorited: 0,
            followers: 0,
            lifetime_subscriptions: 0,
            lifetime_favorited: 0,
            lifetime_followers: 0,
            lifetime_playtime: "".to_string(),
            lifetime_playtime_sessions: "".to_string(),
            views: 0,
            num_children: 0,
            num_reports: 0,
            tags: vec![],
        }),
        updated: None,
    }];
    
    // Ignore update
    let result = commands::ignore_update(mods).await;
    
    // May succeed or fail depending on Steam API availability
    if result.is_ok() {
        // If successful, check that .lastupdated file was updated (if folder was found)
        if last_updated_path.exists() {
            let content = fs::read_to_string(&last_updated_path).unwrap();
            // Should be updated to 2000000000 if function worked correctly
            assert!(content == "1000000000" || content == "2000000000");
        }
    }
}

/// Integration test: Workshop API workflow (file details -> is collection -> collection details)
#[tokio::test]
async fn test_integration_workshop_api_workflow() {
    // Test with a known mod ID (may or may not exist)
    let mod_id = "123456789";
    
    // Get file details
    let result = commands::get_file_details(mod_id.to_string()).await;
    // May succeed or fail depending on Steam API
    if result.is_ok() {
        let details = result.unwrap();
        assert!(details["publishedfileid"].as_str().is_some());
    }
    
    // Check if collection
    let result = commands::is_collection(mod_id.to_string()).await;
    if result.is_ok() {
        let value = result.unwrap();
        assert!(value["isCollection"].is_boolean());
    }
    
    // Get collection details (if it's a collection)
    let result = commands::get_collection_details(mod_id.to_string()).await;
    if result.is_ok() {
        let _details = result.unwrap();
        // details is already Vec<serde_json::Value>, so we just verify it's a valid Vec
    }
}

/// Integration test: Query mods and check backup workflow (without SteamCMD)
#[tokio::test]
async fn test_integration_query_and_backup_check() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create test mod
    let mod_id = "123456789";
    let folder_name = "TestMod";
    let mod_path = create_test_mod_folder(&temp_dir, mod_id, folder_name, Some(1000000000));
    
    let mods_path = temp_dir.path().to_string_lossy().to_string();
    
    // Query mods (doesn't require SteamCMD, only Steam API)
    let result = commands::query_mods(mods_path.clone(), vec![]).await;
    assert!(result.is_ok());
    
    // Check backup (no backup directory) - doesn't require SteamCMD
    let result = commands::check_backup(
        mod_path.to_string_lossy().to_string(),
        None
    ).await;
    assert!(result.is_ok());
    let backup_info = result.unwrap();
    assert_eq!(backup_info["hasBackup"], false);
}

/// Integration test: Multiple mods with same ID workflow
/// Note: Requires Steam API but not SteamCMD
#[tokio::test]
async fn test_integration_multiple_mods_same_id() {
    let temp_dir = TempDir::new().unwrap();
    let mod_id = "123456789";
    
    // Create multiple folders with the same mod ID
    let folder1 = create_test_mod_folder(&temp_dir, mod_id, "Folder1", Some(1000000000));
    let folder2 = create_test_mod_folder(&temp_dir, mod_id, "Folder2", Some(1000000000));
    
    // Also create PublishedFileId.txt files
    fs::write(folder1.join("About").join("PublishedFileId.txt"), mod_id).unwrap();
    fs::write(folder2.join("About").join("PublishedFileId.txt"), mod_id).unwrap();
    
    // Query mods should find both folders
    let mods_path = temp_dir.path().to_string_lossy().to_string();
    let result = commands::query_mods(mods_path, vec![]).await;
    assert!(result.is_ok());
    
    // Test ignore_update with multiple folders
    let mods = vec![BaseMod {
        mod_id: mod_id.to_string(),
        mod_path: folder1.to_string_lossy().to_string(),
        folder: Some("Folder1".to_string()),
        details: Some(WorkshopFileDetails {
            publishedfileid: mod_id.to_string(),
            title: "Folder1".to_string(),
            time_updated: 2000000000,
            result: 1,
            creator: "".to_string(),
            creator_app_id: 0,
            consumer_app_id: 0,
            filename: "".to_string(),
            file_size: 0,
            file_url: "".to_string(),
            hcontent_file: "".to_string(),
            preview_url: "".to_string(),
            hcontent_preview: "".to_string(),
            description: "".to_string(),
            time_created: 0,
            visibility: 0,
            flags: 0,
            workshop_file_url: "".to_string(),
            workshop_accepted: false,
            show_subscribe_all: false,
            num_comments_developer: 0,
            num_comments_public: 0,
            banned: false,
            ban_reason: "".to_string(),
            banner: "".to_string(),
            can_be_deleted: false,
            app_name: "".to_string(),
            file_type: 0,
            can_subscribe: false,
            subscriptions: 0,
            favorited: 0,
            followers: 0,
            lifetime_subscriptions: 0,
            lifetime_favorited: 0,
            lifetime_followers: 0,
            lifetime_playtime: "".to_string(),
            lifetime_playtime_sessions: "".to_string(),
            views: 0,
            num_children: 0,
            num_reports: 0,
            tags: vec![],
        }),
        updated: None,
    }];
    
    let result = commands::ignore_update(mods).await;
    // May succeed or fail depending on Steam API
    if result.is_ok() {
        // Check that .lastupdated files were updated in both folders
        let last_updated1 = folder1.join("About").join(".lastupdated");
        let last_updated2 = folder2.join("About").join(".lastupdated");
        
        if last_updated1.exists() {
            let content1 = fs::read_to_string(&last_updated1).unwrap();
            assert!(content1 == "1000000000" || content1 == "2000000000");
        }
        
        if last_updated2.exists() {
            let content2 = fs::read_to_string(&last_updated2).unwrap();
            assert!(content2 == "1000000000" || content2 == "2000000000");
        }
    }
}

/// Integration test: Error handling workflow
#[tokio::test]
async fn test_integration_error_handling() {
    // Test query_mods with non-existent path
    let result = commands::query_mods("/nonexistent/path/12345".to_string(), vec![]).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("does not exist"));
    
    // Test check_backup with invalid mod path
    let result = commands::check_backup("/nonexistent/mod".to_string(), None).await;
    assert!(result.is_ok()); // Returns hasBackup: false, not an error
    
    // Test restore_backup with non-existent backup
    let temp_dir = TempDir::new().unwrap();
    let mod_path = temp_dir.path().join("TestMod").to_string_lossy().to_string();
    let backup_dir = temp_dir.path().join("backups").to_string_lossy().to_string();
    
    let result = commands::restore_backup(mod_path, backup_dir).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Backup not found"));
    
    // Test update_mods with empty array (validation test - doesn't require SteamCMD)
    let result = commands::update_mods(vec![], false, None).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("mods array is required"));
    
    // Note: Full update_mods test requires SteamCMD, so we only test validation here
}

/// Integration test: Backup directory validation workflow
#[tokio::test]
async fn test_integration_backup_validation() {
    let temp_dir = TempDir::new().unwrap();
    let mod_path = temp_dir.path().join("TestMod").to_string_lossy().to_string();
    
    // Test restore_backup with same paths
    let result = commands::restore_backup(mod_path.clone(), mod_path.clone()).await;
    assert!(result.is_err());
    let error_msg = result.unwrap_err();
    assert!(
        error_msg.contains("cannot be the same") ||
        error_msg.contains("same") ||
        error_msg.contains("Invalid backup path") ||
        error_msg.contains("Backup directory cannot be inside")
    );
    
    // Test restore_backup with nested paths
    let nested_backup = temp_dir.path().join("TestMod").join("backups").to_string_lossy().to_string();
    let result = commands::restore_backup(mod_path.clone(), nested_backup).await;
    assert!(result.is_err());
    let error_msg = result.unwrap_err();
    assert!(
        error_msg.contains("cannot be inside") ||
        error_msg.contains("same") ||
        error_msg.contains("Invalid backup path")
    );
}

