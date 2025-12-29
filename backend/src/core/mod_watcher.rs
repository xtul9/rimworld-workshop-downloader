// File system watcher for mods folder
// Observes the mods folder and emits events when mods are added or removed

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::collections::{HashSet, HashMap};
use tokio::sync::Mutex;
use notify::{Watcher, RecommendedWatcher, RecursiveMode, Event, EventKind};
use tauri::{AppHandle, Emitter};
use crate::core::mod_scanner::{list_installed_mods_fast, query_mod_info, get_mod_last_updated_time, create_workshop_file_details, create_base_mod_from_path};
use crate::services::canonicalize_path_or_fallback;

pub struct ModWatcher {
    watcher: Option<RecommendedWatcher>,
    mods_path: Option<PathBuf>,
    app_handle: Option<AppHandle>,
    known_mods: Arc<Mutex<HashMap<PathBuf, String>>>, // Track folder path -> mod_id mapping to detect additions/removals
    pending_folders: Arc<Mutex<HashSet<PathBuf>>>, // Track folders that might become mods (don't have About/ yet)
    ignored_paths: Arc<Mutex<HashSet<PathBuf>>>, // Track paths to ignore during app operations (updates, restores, etc.)
    periodic_check_handle: Option<tokio::task::JoinHandle<()>>, // Handle for periodic check task to allow cancellation
}

impl ModWatcher {
    pub fn new() -> Self {
        Self {
            watcher: None,
            mods_path: None,
            app_handle: None,
            known_mods: Arc::new(Mutex::new(HashMap::new())),
            pending_folders: Arc::new(Mutex::new(HashSet::new())),
            ignored_paths: Arc::new(Mutex::new(HashSet::new())),
            periodic_check_handle: None,
        }
    }

    /// Ignore events for a specific path (used during app operations like updates/restores)
    pub async fn ignore_path(&self, path: PathBuf) {
        let mut ignored = self.ignored_paths.lock().await;
        use crate::services::canonicalize_path_or_fallback;
        let canonical_path = canonicalize_path_or_fallback(&path);
        ignored.insert(canonical_path);
    }

    /// Stop ignoring events for a specific path
    pub async fn unignore_path(&self, path: PathBuf) {
        let mut ignored = self.ignored_paths.lock().await;
        use crate::services::canonicalize_path_or_fallback;
        let canonical_path = canonicalize_path_or_fallback(&path);
        ignored.remove(&canonical_path);
    }

    /// Start watching the mods folder for changes
    pub async fn start_watching(&mut self, mods_path: PathBuf, app: AppHandle) -> Result<(), String> {
        // Stop existing watcher if any
        self.stop_watching().await;

        self.mods_path = Some(mods_path.clone());
        self.app_handle = Some(app.clone());

        // Get initial list of mods
        // Canonicalize mods_path to handle symlinks consistently
        let canonical_mods_path = mods_path.canonicalize()
            .unwrap_or_else(|_| mods_path.clone());
        
        let initial_mods = list_installed_mods_fast(&canonical_mods_path)
            .await
            .map_err(|e| format!("Failed to list initial mods: {}", e))?;
        
        let initial_mod_map: HashMap<PathBuf, String> = initial_mods
            .iter()
            .filter_map(|m| {
                // Try to canonicalize, but also store original path as fallback
                let mod_path_buf = PathBuf::from(&m.mod_path);
                if let Ok(canon_path) = mod_path_buf.canonicalize() {
                    Some((canon_path, m.mod_id.clone()))
                } else {
                    // Fallback to original path if canonicalize fails
                    Some((mod_path_buf, m.mod_id.clone()))
                }
            })
            .collect();
        
        eprintln!("[ModWatcher] Initialized with {} mod(s)", initial_mod_map.len());
        
        {
            let mut known = self.known_mods.lock().await;
            *known = initial_mod_map;
        }

        // Create channel for file system events
        let (tx, mut rx) = tokio::sync::mpsc::channel(100);
        let app_clone = app.clone();
        let canonical_mods_path_clone = canonical_mods_path.clone();
        let known_mods_clone = self.known_mods.clone();
        let pending_folders_clone = self.pending_folders.clone();
        let ignored_paths_clone = self.ignored_paths.clone();

        // Spawn task to process file system events
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                Self::process_fs_event(event, &app_clone, &canonical_mods_path_clone, &known_mods_clone, &pending_folders_clone, &ignored_paths_clone).await;
            }
        });
        
        // Spawn task to periodically check pending folders (for mods being created)
        let app_clone_retry = app.clone();
        let canonical_mods_path_clone_retry = canonical_mods_path.clone();
        let known_mods_clone_retry = self.known_mods.clone();
        let pending_folders_clone_retry = self.pending_folders.clone();
        let ignored_paths_clone_retry = self.ignored_paths.clone();
        let periodic_check_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                Self::check_pending_folders(&app_clone_retry, &canonical_mods_path_clone_retry, &known_mods_clone_retry, &pending_folders_clone_retry, &ignored_paths_clone_retry).await;
            }
        });
        self.periodic_check_handle = Some(periodic_check_handle);

        // Create watcher
        let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                // Send event to channel (non-blocking)
                let _ = tx.try_send(event);
            }
        })
        .map_err(|e| format!("Failed to create file system watcher: {}", e))?;

        // Watch the mods folder recursively
        // Use canonical path for watching to handle symlinks properly
        watcher.watch(&canonical_mods_path, RecursiveMode::Recursive)
            .map_err(|e| format!("Failed to watch mods folder: {}", e))?;
        
        eprintln!("[ModWatcher] Watching canonical path: {:?}", canonical_mods_path);

        self.watcher = Some(watcher);
        
        Ok(())
    }

    /// Stop watching the mods folder
    pub async fn stop_watching(&mut self) {
        // Cancel periodic check task if it's running
        if let Some(handle) = self.periodic_check_handle.take() {
            handle.abort();
            eprintln!("[ModWatcher] Cancelled periodic check task");
        }
        
        if let Some(watcher) = self.watcher.take() {
            drop(watcher);
            eprintln!("[ModWatcher] Stopped watching mods folder");
        }
        self.mods_path = None;
        self.app_handle = None;
        {
            let mut known = self.known_mods.lock().await;
            known.clear();
        }
        {
            let mut pending = self.pending_folders.lock().await;
            pending.clear();
        }
        {
            let mut ignored = self.ignored_paths.lock().await;
            ignored.clear();
        }
    }

    /// Process file system event and emit mod-added/mod-removed events
    async fn process_fs_event(
        event: Event,
        app: &AppHandle,
        mods_path: &Path,
        known_mods: &Arc<Mutex<HashMap<PathBuf, String>>>,
        pending_folders: &Arc<Mutex<HashSet<PathBuf>>>,
        ignored_paths: &Arc<Mutex<HashSet<PathBuf>>>,
    ) {
        // Filter out events for temporary access test files
        let is_access_test_file = event.paths.iter().any(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|s| s == ".access_test_temp_file")
                .unwrap_or(false)
        });
        
        if is_access_test_file {
            // Silently ignore access test file events
            return;
        }
        
        // First, check if any path is inside an ignored folder (before processing)
        // This prevents logging events for files inside mods being updated
        let ignored = ignored_paths.lock().await;
        let is_inside_ignored = event.paths.iter().any(|p| {
            // For each ignored path, check if event path starts with it
            for ignored_path in ignored.iter() {
                // Try to canonicalize event path first (most reliable)
                if let Ok(canon_p) = p.canonicalize() {
                    if canon_p.starts_with(ignored_path) {
                        return true;
                    }
                }
                // Fallback: check if string representation starts with ignored path
                // This handles cases where canonicalization fails (file being copied)
                let p_str = p.to_string_lossy();
                let ignored_str = ignored_path.to_string_lossy();
                if p_str.starts_with(ignored_str.as_ref()) {
                    return true;
                }
            }
            false
        });
        drop(ignored);
        
        if is_inside_ignored {
            // Silently ignore events inside ignored folders (app operations in progress)
            return;
        }
        
        // Check if the event is for a directory (mod folder) directly under mods_path
        // Note: mods_path is already canonicalized, so we should canonicalize event paths too
        // For Modify(Name(From)) events, the folder might not exist anymore, so we need special handling
        let paths: Vec<PathBuf> = event.paths.into_iter()
            .filter_map(|p| {
                // For Modify(Name(From)) and Remove events, folder might not exist
                // So we check parent first before trying to canonicalize
                let is_removal_event = matches!(event.kind, EventKind::Remove(_)) ||
                    matches!(event.kind, EventKind::Modify(notify::event::ModifyKind::Name(notify::event::RenameMode::From)));
                
                if is_removal_event {
                    // For removal events, check parent without canonicalizing the path itself
                    // (because it might not exist)
                    if let Some(parent) = p.parent() {
                        // Try to canonicalize parent to compare with mods_path
                        if let Ok(canon_parent) = parent.canonicalize() {
                            if canon_parent == mods_path {
                                // Path is a direct child - use the original path or try to reconstruct canonical
                                // Try to canonicalize if possible, otherwise use original
                                if let Ok(canon_p) = p.canonicalize() {
                                    return Some(canon_p);
                                } else {
                                    // Folder doesn't exist, reconstruct canonical path from parent + filename
                                    if let Some(file_name) = p.file_name() {
                                        let reconstructed = mods_path.join(file_name);
                                        return Some(reconstructed);
                                    }
                                }
                            }
                        }
                    }
                    return None;
                }
                
                // For other events (Create, Modify(Name(To)), etc.), folder should exist
                // Try to canonicalize the path
                if let Ok(canonical_p) = p.canonicalize() {
                    // Check if this is a direct child of mods_path
                    if let Some(parent) = canonical_p.parent() {
                        if parent != mods_path {
                            return None;
                        }
                        
                        // For Create events, verify it's actually a directory
                        if matches!(event.kind, EventKind::Create(_)) {
                            if let Ok(metadata) = std::fs::metadata(&canonical_p) {
                                if !metadata.is_dir() {
                                    return None;
                                }
                            }
                        }
                        
                        return Some(canonical_p);
                    }
                }
                None
            })
            .collect();

        if paths.is_empty() {
            eprintln!("[ModWatcher] No valid paths found for event");
            return;
        }

        // Check if any of the paths are being ignored (app operations in progress)
        // This is a second check after filtering to direct children
        let ignored = ignored_paths.lock().await;
        let filtered_paths: Vec<PathBuf> = paths.into_iter()
            .filter(|p| {
                // Check if path or any parent is ignored
                let mut current = p.clone();
                loop {
                    if ignored.contains(&current) {
                        return false;
                    }
                    if let Some(parent) = current.parent() {
                        if parent == mods_path {
                            break; // Reached mods_path, stop checking
                        }
                        current = parent.to_path_buf();
                    } else {
                        break;
                    }
                }
                true
            })
            .collect();
        drop(ignored);

        if filtered_paths.is_empty() {
            eprintln!("[ModWatcher] All paths ignored (app operations in progress)");
            return;
        }

        eprintln!("[ModWatcher] Processing {} path(s) for event {:?}", filtered_paths.len(), event.kind);

        // Small delay to allow file system operations to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        match event.kind {
            EventKind::Create(_) => {
                // New folder created - check if it's a mod
                for folder_path in &filtered_paths {
                    Self::check_single_folder(folder_path, app, mods_path, known_mods, pending_folders, false).await;
                }
            }
            EventKind::Remove(_) => {
                // Folder removed - check if it was a known mod
                eprintln!("[ModWatcher] Remove event detected for paths: {:?}", filtered_paths);
                let mut known = known_mods.lock().await;
                let mut pending = pending_folders.lock().await;
                
                eprintln!("[ModWatcher] Known mods count: {}", known.len());
                
                // Remove from pending folders
                for folder_path in &filtered_paths {
                    pending.remove(folder_path);
                    
                    // Try to find matching path in known_mods
                    // We need to handle both symlink paths and canonical paths
                    let mut mod_id_to_remove: Option<String> = None;
                    let mut path_to_remove: Option<PathBuf> = None;
                    
                    // Try multiple matching strategies
                    // 1. Try exact match (as-is)
                    if let Some(mod_id) = known.get(folder_path) {
                        mod_id_to_remove = Some(mod_id.clone());
                        path_to_remove = Some(folder_path.clone());
                    }
                    
                    // 2. Try canonical path if available (before removal)
                    if mod_id_to_remove.is_none() {
                        // Try to canonicalize parent and reconstruct path
                        if let Some(parent) = folder_path.parent() {
                            if let Ok(canon_parent) = parent.canonicalize() {
                                if let Some(folder_name) = folder_path.file_name() {
                                    let canon_path = canon_parent.join(folder_name);
                                    if let Some(mod_id) = known.get(&canon_path) {
                                        mod_id_to_remove = Some(mod_id.clone());
                                        path_to_remove = Some(canon_path);
                                    }
                                }
                            }
                        }
                    }
                    
                    // 3. Try comparing string paths (normalize separators and case)
                    if mod_id_to_remove.is_none() {
                        let folder_path_str = folder_path.to_string_lossy().to_string();
                        let folder_name = folder_path.file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.to_string());
                        
                        for (stored_path, stored_mod_id) in known.iter() {
                            let stored_path_str = stored_path.to_string_lossy().to_string();
                            let stored_name = stored_path.file_name()
                                .and_then(|n| n.to_str())
                                .map(|s| s.to_string());
                            
                            // Compare by folder name (most reliable for symlinks)
                            if let (Some(fname), Some(sname)) = (&folder_name, &stored_name) {
                                if fname == sname {
                                    mod_id_to_remove = Some(stored_mod_id.clone());
                                    path_to_remove = Some(stored_path.clone());
                                    break;
                                }
                            }
                            
                            // Also try full path comparison (normalize separators)
                            let normalized_folder = folder_path_str.replace('\\', "/");
                            let normalized_stored = stored_path_str.replace('\\', "/");
                            if normalized_folder == normalized_stored {
                                mod_id_to_remove = Some(stored_mod_id.clone());
                                path_to_remove = Some(stored_path.clone());
                                break;
                            }
                        }
                    }
                    
                    // Remove from known_mods and emit event if found
                    if let (Some(mod_id), Some(path)) = (mod_id_to_remove, path_to_remove) {
                        known.remove(&path);
                        eprintln!("[ModWatcher] Mod removed: {} (folder: {:?}, stored path: {:?})", mod_id, folder_path, path);
                        let _ = app.emit("mod-removed", serde_json::json!({
                            "modId": mod_id,
                        }));
                    } else {
                        eprintln!("[ModWatcher] Folder removed but not found in known_mods: {:?}", folder_path);
                    }
                }
            }
            EventKind::Modify(modify_kind) => {
                // Folder modified - could be:
                // 1. Name(From) - folder moved/deleted (folder doesn't exist)
                // 2. Name(To) - folder restored/created (folder exists)
                // 3. Other modifications (folder exists)
                
                match modify_kind {
                    notify::event::ModifyKind::Name(notify::event::RenameMode::From) => {
                        // Folder was moved/deleted - treat as removal
                        eprintln!("[ModWatcher] Modify(Name(From)) event - treating as removal");
                        let mut known = known_mods.lock().await;
                        let mut pending = pending_folders.lock().await;
                        
                        for folder_path in &filtered_paths {
                            pending.remove(folder_path);
                            
                            // Find and remove from known_mods
                            let mut mod_id_to_remove: Option<String> = None;
                            let mut path_to_remove: Option<PathBuf> = None;
                            
                            // Try direct lookup first (most reliable)
                            if let Some(mod_id) = known.get(folder_path) {
                                mod_id_to_remove = Some(mod_id.clone());
                                path_to_remove = Some(folder_path.clone());
                            } else {
                                // Fallback: match by folder name
                                let folder_name = folder_path.file_name()
                                    .and_then(|n| n.to_str())
                                    .map(|s| s.to_string());
                                
                                if let Some(fname) = folder_name {
                                    for (stored_path, stored_mod_id) in known.iter() {
                                        if let Some(sname) = stored_path.file_name().and_then(|n| n.to_str()) {
                                            if sname == fname {
                                                mod_id_to_remove = Some(stored_mod_id.clone());
                                                path_to_remove = Some(stored_path.clone());
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            
                            if let (Some(mod_id), Some(path)) = (mod_id_to_remove, path_to_remove) {
                                known.remove(&path);
                                eprintln!("[ModWatcher] Mod removed (via Modify(Name(From))): {} (folder: {:?})", mod_id, folder_path);
                                let _ = app.emit("mod-removed", serde_json::json!({
                                    "modId": mod_id,
                                }));
                            } else {
                                eprintln!("[ModWatcher] Modify(Name(From)) event but not found in known_mods: {:?}", folder_path);
                            }
                        }
                    }
                    notify::event::ModifyKind::Name(notify::event::RenameMode::To) => {
                        // Folder was restored/created - check if it's a mod
                        eprintln!("[ModWatcher] Modify(Name(To)) event - folder restored/created");
                        for folder_path in &filtered_paths {
                            // Check if folder exists and is a mod
                            if folder_path.exists() {
                                // Check if it's already in known_mods (might have been restored)
                                let known = known_mods.lock().await;
                                let is_known = known.contains_key(folder_path);
                                drop(known);
                                
                                if !is_known {
                                    // New or restored folder - check if it's a mod
                                    // Use restored version which sets current time for proper sorting
                                    Self::check_single_folder_restored(folder_path, app, mods_path, known_mods, pending_folders).await;
                                }
                                // If already known, the mod is already in the list - no need to update
                            }
                        }
                    }
                    _ => {
                        // Other modifications - might be adding About/ folder
                        for folder_path in &filtered_paths {
                            if folder_path.exists() {
                                let pending = pending_folders.lock().await;
                                if pending.contains(folder_path) {
                                    // This folder is pending - check if it's now a mod
                                    drop(pending);
                                    Self::check_single_folder(folder_path, app, mods_path, known_mods, pending_folders, false).await;
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    
    /// Check a single folder to see if it's a mod
    /// `use_current_time` - if true, use current time for time_updated (for restored mods)
    ///                      if false, use time from folder modification time or .lastupdated file
    async fn check_single_folder(
        folder_path: &Path,
        app: &AppHandle,
        _mods_path: &Path,
        known_mods: &Arc<Mutex<HashMap<PathBuf, String>>>,
        pending_folders: &Arc<Mutex<HashSet<PathBuf>>>,
        use_current_time: bool,
    ) {
        // Query mod info for this specific folder (use spawn_blocking to avoid Send issues)
        let folder_path_clone = folder_path.to_path_buf();
        let mod_info_result = tokio::task::spawn_blocking(move || {
            query_mod_info(&folder_path_clone).map_err(|e| e.to_string())
        }).await;
        
        let mod_info = match mod_info_result {
            Ok(Ok(Some(info))) => info,
            Ok(Ok(None)) => {
                // Not a mod yet - add to pending folders for retry
                let mut pending = pending_folders.lock().await;
                pending.insert(folder_path.to_path_buf());
                eprintln!("[ModWatcher] Folder {:?} is not a mod yet, adding to pending", folder_path);
                return;
            }
            Ok(Err(e)) => {
                eprintln!("[ModWatcher] Error querying mod info for {:?}: {}", folder_path, e);
                return;
            }
            Err(e) => {
                eprintln!("[ModWatcher] Task error for {:?}: {}", folder_path, e);
                return;
            }
        };
        
        // Remove from pending folders if it was there
        {
            let mut pending = pending_folders.lock().await;
            pending.remove(folder_path);
        }
        
        // Try to canonicalize path for consistent mapping
        let canonical_path = canonicalize_path_or_fallback(folder_path);
        
        // Check if this mod is already known
        let mut known = known_mods.lock().await;
        if let Some(existing_mod_id) = known.get(&canonical_path) {
            if existing_mod_id == &mod_info.mod_id {
                // Already known with same mod_id, skip
                return;
            }
            // Different mod_id for same path - update it
        }
        
        // Get folder name for title
        let folder_name = folder_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| mod_info.mod_id.clone());
        
        // Create WorkshopFileDetails only for Steam mods (non-steam mods don't have time_updated)
        // Non-steam mods will be sorted by name on the frontend
        let details = if !mod_info.is_non_steam {
            let time_updated = if use_current_time {
                // Use current time for restored mods so they appear at the top
                tokio::task::spawn_blocking(move || {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .map(|duration| duration.as_secs() as i64)
                        .unwrap_or(0)
                }).await.ok().unwrap_or(0)
            } else {
                // Get time_updated from folder modification time or .lastupdated file
                let folder_path_clone_for_time = folder_path.to_path_buf();
                tokio::task::spawn_blocking(move || {
                    get_mod_last_updated_time(&folder_path_clone_for_time)
                        .ok()
                        .and_then(|system_time| {
                            system_time.duration_since(std::time::UNIX_EPOCH).ok()
                                .map(|duration| duration.as_secs() as i64)
                        })
                        .unwrap_or(0)
                }).await.ok().unwrap_or(0)
            };
            
            if time_updated > 0 {
                Some(create_workshop_file_details(&mod_info.mod_id, folder_name.clone(), time_updated))
            } else {
                None
            }
        } else {
            None
        };
        
        // Create BaseMod from this folder
        let base_mod = create_base_mod_from_path(
            mod_info.mod_id.clone(),
            folder_path,
            details,
            mod_info.is_non_steam,
        );
        
        // Add to known mods (map folder path to mod_id)
        known.insert(canonical_path, mod_info.mod_id.clone());
        drop(known);
        
        let log_message = if use_current_time {
            "[ModWatcher] Mod restored: {}"
        } else {
            "[ModWatcher] Mod added: {}"
        };
        eprintln!("{}", log_message.replace("{}", &mod_info.mod_id));
        let _ = app.emit("mod-added", serde_json::json!({
            "modId": mod_info.mod_id,
            "mod": base_mod,
        }));
    }
    
    /// Check a single folder that was restored (use current time for sorting)
    async fn check_single_folder_restored(
        folder_path: &Path,
        app: &AppHandle,
        mods_path: &Path,
        known_mods: &Arc<Mutex<HashMap<PathBuf, String>>>,
        pending_folders: &Arc<Mutex<HashSet<PathBuf>>>,
    ) {
        Self::check_single_folder(folder_path, app, mods_path, known_mods, pending_folders, true).await;
    }
    
    
    /// Periodically check pending folders to see if they've become mods
    /// Also verify that all known mods still exist
    async fn check_pending_folders(
        app: &AppHandle,
        mods_path: &Path,
        known_mods: &Arc<Mutex<HashMap<PathBuf, String>>>,
        pending_folders: &Arc<Mutex<HashSet<PathBuf>>>,
        ignored_paths: &Arc<Mutex<HashSet<PathBuf>>>,
    ) {
        // Check pending folders
        let folders_to_check: Vec<PathBuf> = {
            let pending = pending_folders.lock().await;
            pending.iter().cloned().collect()
        };
        
        for folder_path in folders_to_check {
            // Check if folder still exists
            if !folder_path.exists() {
                let mut pending = pending_folders.lock().await;
                pending.remove(&folder_path);
                continue;
            }
            
            // Check if path is ignored (app operation in progress)
            let ignored_guard: tokio::sync::MutexGuard<'_, HashSet<PathBuf>> = ignored_paths.lock().await;
            if ignored_guard.contains(&folder_path) {
                drop(ignored_guard);
                continue; // Skip if being ignored
            }
            drop(ignored_guard);
            
            Self::check_single_folder(&folder_path, app, mods_path, known_mods, pending_folders, false).await;
        }
        
        // Verify all known mods still exist (handles cases where events were missed)
        let mods_to_verify: Vec<(PathBuf, String)> = {
            let known = known_mods.lock().await;
            known.iter().map(|(path, mod_id)| (path.clone(), mod_id.clone())).collect()
        };
        
        let mut known = known_mods.lock().await;
        let mut removed_mods = Vec::new();
        
        for (folder_path, mod_id) in mods_to_verify {
            if !folder_path.exists() {
                eprintln!("[ModWatcher] Periodic check: Mod {} folder no longer exists: {:?}", mod_id, folder_path);
                removed_mods.push((folder_path, mod_id));
            }
        }
        
        // Remove non-existent mods and emit events
        for (folder_path, mod_id) in removed_mods {
            known.remove(&folder_path);
            eprintln!("[ModWatcher] Mod removed (periodic check): {} (folder: {:?})", mod_id, folder_path);
            let _ = app.emit("mod-removed", serde_json::json!({
                "modId": mod_id,
            }));
        }
    }
}

impl Drop for ModWatcher {
    fn drop(&mut self) {
        // Stop watching on drop - perform minimal synchronous cleanup
        // The watcher itself can be dropped synchronously
        if let Some(watcher) = self.watcher.take() {
            drop(watcher);
            eprintln!("[ModWatcher] Stopped watching mods folder (on drop)");
        }
        
        // Clear synchronous fields
        self.mods_path = None;
        self.app_handle = None;
        
        // For async structures, try to clean up if we're in an async context
        // Otherwise, they'll be cleaned up when the Arc is dropped
        let known_mods = self.known_mods.clone();
        let pending_folders = self.pending_folders.clone();
        let ignored_paths = self.ignored_paths.clone();
        
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            // We're in an async context, spawn cleanup tasks
            handle.spawn(async move {
                let mut known = known_mods.lock().await;
                known.clear();
            });
            handle.spawn(async move {
                let mut pending = pending_folders.lock().await;
                pending.clear();
            });
            handle.spawn(async move {
                let mut ignored = ignored_paths.lock().await;
                ignored.clear();
            });
        }
        // If we're not in an async context, the structures will be cleaned up
        // when the Arc is dropped, which is acceptable
    }
}

