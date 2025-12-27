use std::path::PathBuf;
use std::fs;
use std::time::Duration;
use tokio::time::sleep;
use tokio::process::Command;
use futures;
use tauri::{AppHandle, Emitter};
use notify::{Watcher, RecommendedWatcher, RecursiveMode, Event};
use std::sync::{mpsc, Arc, Mutex};

pub struct Downloader {
    steamcmd_path: PathBuf,
    download_path: PathBuf,
    active_downloads: std::collections::HashSet<String>,
}

impl Downloader {
    pub fn new(steamcmd_path: Option<PathBuf>) -> Self {
        let steamcmd_path = steamcmd_path.unwrap_or_else(|| PathBuf::from("steamcmd"));
        let download_path = steamcmd_path.join("steamapps").join("workshop").join("content").join("294100");
        
        Self {
            steamcmd_path,
            download_path,
            active_downloads: std::collections::HashSet::new(),
        }
    }

    /// Get the download path where mods are downloaded
    pub fn download_path(&self) -> &PathBuf {
        &self.download_path
    }

    /// Find SteamCMD executable from application resources or PATH
    pub async fn find_steamcmd_executable(&self) -> Result<PathBuf, String> {
        // Try to find in application resources first
        if let Some(resource_path) = self.find_steamcmd_from_resources().await? {
            return Ok(resource_path);
        }

        // Try local path
        let steamcmd_exe = if cfg!(target_os = "windows") {
            "steamcmd.exe"
        } else {
            "steamcmd"
        };
        
        let local_path = self.steamcmd_path.join(steamcmd_exe);
        if local_path.exists() {
            return Ok(local_path);
        }

        // Try PATH
        let which_cmd = if cfg!(target_os = "windows") { "where" } else { "which" };
        if let Ok(output) = Command::new(which_cmd)
            .arg(steamcmd_exe)
            .output()
            .await
        {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout);
                let path = PathBuf::from(path_str.trim().lines().next().unwrap_or(""));
                if path.exists() {
                    return Ok(path);
                }
            }
        }

        Err(format!("SteamCMD not found in resources, at {:?}, or in PATH", local_path))
    }

    /// Find SteamCMD from application resources (bundled with app)
    async fn find_steamcmd_from_resources(&self) -> Result<Option<PathBuf>, String> {
        let is_windows = cfg!(target_os = "windows");
        let steamcmd_exe = if is_windows { "steamcmd.exe" } else { "steamcmd" };
        
        // Determine target triple for current platform
        let target_triple = if is_windows {
            "x86_64-pc-windows-msvc"
        } else if cfg!(target_os = "macos") {
            if cfg!(target_arch = "aarch64") {
                "aarch64-apple-darwin"
            } else {
                "x86_64-apple-darwin"
            }
        } else {
            "x86_64-unknown-linux-gnu"
        };
        
        let steamcmd_name_with_suffix = if is_windows {
            format!("steamcmd-{}.exe", target_triple)
        } else {
            format!("steamcmd-{}", target_triple)
        };
        
        // Possible paths where SteamCMD might be located
        let exe_path = std::env::current_exe()
            .map_err(|e| format!("Failed to get current executable path: {}", e))?;
        let exe_dir = exe_path.parent().ok_or_else(|| "Cannot get executable directory".to_string())?;
        
        let possible_paths = vec![
            exe_dir.join(&steamcmd_name_with_suffix),
            exe_dir.join("resources").join(&steamcmd_name_with_suffix),
            exe_dir.join("..").join(&steamcmd_name_with_suffix),
            exe_dir.join("..").join("resources").join(&steamcmd_name_with_suffix),
            PathBuf::from("bin").join("steamcmd").join(steamcmd_exe),
            self.steamcmd_path.join(steamcmd_exe),
        ];
        
        for path in possible_paths {
            if path.exists() {
                return Ok(Some(path));
            }
        }
        
        Ok(None)
    }

    /// Balance mods across instances using round-robin (simple fallback)
    fn balance_mods_round_robin(mod_ids: &[String], num_instances: usize) -> Vec<Vec<String>> {
        let mut batches: Vec<Vec<String>> = vec![Vec::new(); num_instances];
        for (idx, mod_id) in mod_ids.iter().enumerate() {
            batches[idx % num_instances].push(mod_id.clone());
        }
        batches
    }

    /// Balance mods across instances by size (load balancing)
    /// Uses a greedy algorithm: assign each mod to the instance with the least current load
    fn balance_mods_by_size(
        mod_ids: &[String],
        mod_sizes: &std::collections::HashMap<String, u64>,
        num_instances: usize,
    ) -> Vec<Vec<String>> {
        // Sort mods by size (largest first) for better balancing
        let mut mods_with_sizes: Vec<(String, u64)> = mod_ids
            .iter()
            .map(|mod_id| {
                let size = mod_sizes.get(mod_id).copied().unwrap_or(0);
                (mod_id.clone(), size)
            })
            .collect();
        
        // Sort by size descending (largest first) for better load balancing
        mods_with_sizes.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Track current load for each instance
        let mut instance_loads: Vec<u64> = vec![0; num_instances];
        let mut batches: Vec<Vec<String>> = vec![Vec::new(); num_instances];
        
        // Greedy assignment: assign each mod to the instance with the least current load
        for (mod_id, size) in mods_with_sizes {
            // Find instance with minimum load
            let min_load_idx = instance_loads
                .iter()
                .enumerate()
                .min_by_key(|(_, &load)| load)
                .map(|(idx, _)| idx)
                .unwrap_or(0);
            
            // Assign mod to this instance
            batches[min_load_idx].push(mod_id);
            instance_loads[min_load_idx] += size;
        }
        
        // Log load distribution for debugging
        eprintln!("[Downloader] Load distribution (size-based):");
        for (idx, load) in instance_loads.iter().enumerate() {
            eprintln!("[Downloader]   Instance {}: {} mod(s), {} bytes", idx, batches[idx].len(), load);
        }
        
        batches
    }

    /// Download mods using SteamCMD with parallel instances for better performance
    /// For small batches (<=4 mods), uses single instance. For larger batches, uses up to 4 parallel instances.
    /// If mod_sizes is provided, mods are balanced by size across instances.
    pub async fn download_mods(&mut self, mod_ids: &[String], app: Option<&AppHandle>) -> Result<Vec<DownloadedMod>, String> {
        self.download_mods_with_sizes(mod_ids, None, app).await
    }

    /// Download mods with optional size information for load balancing
    pub async fn download_mods_with_sizes(
        &mut self,
        mod_ids: &[String],
        mod_sizes: Option<&std::collections::HashMap<String, u64>>,
        app: Option<&AppHandle>,
    ) -> Result<Vec<DownloadedMod>, String> {
        if mod_ids.is_empty() {
            return Ok(vec![]);
        }

        // Delete appworkshop file if it exists
        let appworkshop_path = self.steamcmd_path
            .join("steamapps")
            .join("workshop")
            .join("appworkshop_294100.acf");
        let _ = fs::remove_file(&appworkshop_path);

        // Ensure download directory exists
        fs::create_dir_all(&self.download_path)
            .map_err(|e| format!("Failed to create download directory: {}", e))?;

        const MAX_PARALLEL_INSTANCES: usize = 4; // Hard limit - never exceed this

        // Calculate number of instances: use as many as possible (up to MAX_PARALLEL_INSTANCES)
        // For 3+ mods, always use parallel instances to maximize throughput
        let num_instances = std::cmp::min(mod_ids.len(), MAX_PARALLEL_INSTANCES);
        
        // Balance mods across instances by size if sizes are available
        let batches = if let Some(sizes) = mod_sizes {
            eprintln!("[Downloader] Using {} parallel SteamCMD instances for {} mod(s) (size-based load balancing)", num_instances, mod_ids.len());
            Self::balance_mods_by_size(mod_ids, sizes, num_instances)
        } else {
            eprintln!("[Downloader] Using {} parallel SteamCMD instances for {} mod(s) (round-robin distribution)", num_instances, mod_ids.len());
            // Fallback to simple round-robin if no size information
            Self::balance_mods_round_robin(mod_ids, num_instances)
        };
        
        // Log batch distribution
        for (batch_idx, batch) in batches.iter().enumerate() {
            if !batch.is_empty() {
                eprintln!("[Downloader] Instance {}: {} mod(s)", batch_idx, batch.len());
            }
        }
        
        let mut batch_futures = Vec::new();
        let steamcmd_executable = self.find_steamcmd_executable().await?;
        
        for (batch_idx, batch) in batches.into_iter().enumerate() {
            if batch.is_empty() {
                continue;
            }
            
            let steamcmd_path = self.steamcmd_path.clone();
            let download_path = self.download_path.clone();
            
            let future = Self::download_mods_batch(
                steamcmd_executable.clone(),
                steamcmd_path,
                download_path,
                batch,
                batch_idx,
                app.cloned()
            );
            batch_futures.push(future);
        }
        
        // Wait for all batches to complete in parallel
        let batch_results = futures::future::join_all(batch_futures).await;
        let mut all_downloaded_mods = Vec::new();
        let mut success_count = 0;
        let mut failure_count = 0;
        
        for (batch_idx, result) in batch_results.into_iter().enumerate() {
            match result {
                Ok(mods) => {
                    let mods_count = mods.len();
                    success_count += 1;
                    all_downloaded_mods.extend(mods);
                    eprintln!("[Downloader] Instance {}: completed successfully ({} mod(s))", batch_idx, mods_count);
                }
                Err(e) => {
                    failure_count += 1;
                    eprintln!("[Downloader] Instance {}: failed - {}", batch_idx, e);
                }
            }
        }
        
        eprintln!("[Downloader] All instances completed: {} succeeded, {} failed, {} total mod(s) downloaded", success_count, failure_count, all_downloaded_mods.len());
        Ok(all_downloaded_mods)
    }

    /// Download a batch of mods using a single SteamCMD instance (static version for parallel execution)
    async fn download_mods_batch(
        steamcmd_executable: PathBuf,
        steamcmd_path: PathBuf,
        download_path: PathBuf,
        mod_ids: Vec<String>,
        batch_idx: usize,
        app: Option<AppHandle>,
    ) -> Result<Vec<DownloadedMod>, String> {
        eprintln!("[Downloader] Instance {}: starting download", batch_idx);

        // Get absolute paths
        let steamcmd_path_absolute = if steamcmd_path.is_absolute() {
            steamcmd_path.clone()
        } else {
            let current_dir = std::env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;
            current_dir.join(&steamcmd_path)
        };
        
        let download_path_absolute = if download_path.is_absolute() {
            download_path.clone()
        } else {
            let current_dir = std::env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;
            current_dir.join(&download_path)
        };

        // Create unique script file for this batch
        let script_path = steamcmd_path.join(format!("run_batch_{}.txt", batch_idx));
        let mut script_lines = vec![
            format!("force_install_dir \"{}\"", steamcmd_path_absolute.to_string_lossy()),
            "login anonymous".to_string(),
        ];
        
        for mod_id in &mod_ids {
            script_lines.push(format!("workshop_download_item 294100 {}", mod_id));
        }
        
        script_lines.push("quit".to_string());
        let script_content = script_lines.join("\n") + "\n";
        
        fs::write(&script_path, script_content)
            .map_err(|e| format!("Failed to write SteamCMD script: {}", e))?;

        let script_path_absolute = if script_path.is_absolute() {
            script_path.clone()
        } else {
            let current_dir = std::env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;
            current_dir.join(&script_path)
        };

        // Start watching folders before starting download
        let mut download_promises = Vec::new();
        for mod_id in &mod_ids {
            let mod_download_path = download_path_absolute.join(mod_id.clone());
            let mod_id_clone = mod_id.clone();
            let app_clone = app.clone();
            download_promises.push(Self::wait_for_mod_download_static(mod_download_path, mod_id_clone, app_clone));
        }

        // Start SteamCMD process
        let mut steamcmd_process = Command::new(&steamcmd_executable)
            .arg("+runscript")
            .arg(&script_path_absolute)
            .current_dir(&steamcmd_path_absolute)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn SteamCMD: {}", e))?;

        // Read output in background (simplified - just consume it)
        let stdout = steamcmd_process.stdout.take();
        let stderr = steamcmd_process.stderr.take();
        
        let _stdout_task = if let Some(stdout) = stdout {
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(_line)) = lines.next_line().await {
                    // Output is logged but not critical for batch operations
                }
            })
        } else {
            tokio::spawn(async {})
        };

        let _stderr_task = if let Some(stderr) = stderr {
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(_line)) = lines.next_line().await {
                    // Output is logged but not critical for batch operations
                }
            })
        } else {
            tokio::spawn(async {})
        };

        // Wait for SteamCMD to start
        sleep(Duration::from_secs(2)).await;

        // Wait for SteamCMD to exit
        let _status = steamcmd_process.wait().await
            .map_err(|e| format!("Failed to wait for SteamCMD: {}", e))?;

        // Wait a bit for file system operations
        sleep(Duration::from_secs(1)).await;

        // Wait for all mod downloads to be detected in parallel
        let download_results = futures::future::join_all(download_promises).await;
        let mut downloaded_mods = Vec::new();
        for result in download_results {
            if let Ok(Some(mod_info)) = result {
                downloaded_mods.push(mod_info);
            }
        }

        // Clean up script file
        let _ = fs::remove_file(&script_path);

        Ok(downloaded_mods)
    }

    /// Wait for a mod to be downloaded by watching the download folder (static version for Send)
    async fn wait_for_mod_download_static(
        mod_download_path: PathBuf,
        mod_id: String,
        app: Option<AppHandle>,
    ) -> Result<Option<DownloadedMod>, String> {
        let timeout = Duration::from_secs(600); // 10 minutes timeout
        let start_time = std::time::Instant::now();
        
        // First, check if mod is already downloaded (race condition protection)
        if let Ok(metadata) = fs::metadata(&mod_download_path) {
            if metadata.is_dir() {
                if let Ok(entries) = fs::read_dir(&mod_download_path) {
                    if entries.take(1).count() > 0 {
                        return Self::create_downloaded_mod_result(mod_download_path, mod_id, app);
                    }
                }
            }
        }
        
        // Get parent directory to watch (the workshop content folder)
        let watch_path = mod_download_path.parent()
            .ok_or_else(|| "Cannot get parent directory for watching".to_string())?;
        
        // Create channel for file system events
        let (tx, rx) = mpsc::channel();
        let rx_shared = Arc::new(Mutex::new(rx));
        
        // Create watcher with minimal delay for faster detection
        let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx.send(event);
            }
        })
        .map_err(|e| format!("Failed to create file system watcher: {}", e))?;
        
        // Watch the parent directory (non-recursive, we only care about direct children)
        watcher.watch(watch_path, RecursiveMode::NonRecursive)
            .map_err(|e| format!("Failed to watch directory: {}", e))?;
        
        // Spawn a task to handle file system events
        let mod_download_path_clone = mod_download_path.clone();
        let mod_id_clone = mod_id.clone();
        let app_clone = app.clone();
        let rx_for_task = rx_shared.clone();
        let watch_task = tokio::spawn(async move {
            loop {
                // Check for timeout
                if start_time.elapsed() > timeout {
                    return Ok(None);
                }
                
                // Receive file system event with timeout using spawn_blocking
                let rx_clone = rx_for_task.clone();
                let event_result = tokio::task::spawn_blocking(move || {
                    let rx_guard = rx_clone.lock().unwrap();
                    rx_guard.recv_timeout(Duration::from_secs(2))
                }).await;
                
                match event_result {
                    Ok(Ok(event)) => {
                        // Check if the event is related to our mod folder
                        if event.paths.iter().any(|p: &PathBuf| p == &mod_download_path_clone || 
                            p.parent() == Some(&mod_download_path_clone)) {
                            
                            // Check if mod folder exists and has content
                            if let Ok(metadata) = fs::metadata(&mod_download_path_clone) {
                                if metadata.is_dir() {
                                    if let Ok(entries) = fs::read_dir(&mod_download_path_clone) {
                                        if entries.take(1).count() > 0 {
                                            return Self::create_downloaded_mod_result(
                                                mod_download_path_clone, 
                                                mod_id_clone, 
                                                app_clone
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(Err(mpsc::RecvTimeoutError::Timeout)) => {
                        // Timeout - check if mod exists anyway (fallback polling)
                        if let Ok(metadata) = fs::metadata(&mod_download_path_clone) {
                            if metadata.is_dir() {
                                if let Ok(entries) = fs::read_dir(&mod_download_path_clone) {
                                    if entries.take(1).count() > 0 {
                                        return Self::create_downloaded_mod_result(
                                            mod_download_path_clone, 
                                            mod_id_clone, 
                                            app_clone
                                        );
                                    }
                                }
                            }
                        }
                    }
                    Ok(Err(mpsc::RecvTimeoutError::Disconnected)) => {
                        // Channel closed, watcher stopped
                        break;
                    }
                    Err(_) => {
                        // Task join error
                        break;
                    }
                }
            }
            
            // Final check before timeout
            if let Ok(metadata) = fs::metadata(&mod_download_path_clone) {
                if metadata.is_dir() {
                    if let Ok(entries) = fs::read_dir(&mod_download_path_clone) {
                        if entries.take(1).count() > 0 {
                            return Self::create_downloaded_mod_result(
                                mod_download_path_clone, 
                                mod_id_clone, 
                                app_clone
                            );
                        }
                    }
                }
            }
            
            Ok(None)
        });
        
        // Wait for either the watch task to complete or timeout
        let result = tokio::time::timeout(timeout, watch_task).await;
        
        // Clean up watcher
        drop(watcher);
        
        match result {
            Ok(Ok(mod_result)) => mod_result,
            Ok(Err(e)) => Err(format!("Watch task error: {}", e)),
            Err(_) => {
                // Timeout - final check
                if let Ok(metadata) = fs::metadata(&mod_download_path) {
                    if metadata.is_dir() {
                        if let Ok(entries) = fs::read_dir(&mod_download_path) {
                            if entries.take(1).count() > 0 {
                                return Self::create_downloaded_mod_result(mod_download_path, mod_id, app);
                            }
                        }
                    }
                }
                Ok(None)
            }
        }
    }
    
    /// Helper function to create DownloadedMod result and emit event
    fn create_downloaded_mod_result(
        mod_download_path: PathBuf,
        mod_id: String,
        app: Option<AppHandle>,
    ) -> Result<Option<DownloadedMod>, String> {
        let folder = mod_download_path.file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());
        
        // Emit event for downloaded mod
        if let Some(app_handle) = &app {
            let _ = app_handle.emit("mod-downloaded", serde_json::json!({
                "modId": mod_id,
            }));
        }
        
        Ok(Some(DownloadedMod {
            mod_id: mod_id.clone(),
            mod_path: mod_download_path,
            folder,
        }))
    }

    /// Check if a mod is currently being downloaded
    pub fn is_downloading(&self, mod_id: &str) -> bool {
        self.active_downloads.contains(mod_id)
    }

    /// Mark a mod as downloading
    pub fn mark_downloading(&mut self, mod_id: String) {
        self.active_downloads.insert(mod_id);
    }

    /// Mark a mod as finished downloading
    pub fn mark_downloaded(&mut self, mod_id: &str) {
        self.active_downloads.remove(mod_id);
    }
}

#[derive(Debug, Clone)]
pub struct DownloadedMod {
    pub mod_id: String,
    pub mod_path: PathBuf,
    pub folder: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_is_downloading() {
        let mut downloader = Downloader::new(None);
        assert!(!downloader.is_downloading("123456789"));
        
        downloader.mark_downloading("123456789".to_string());
        assert!(downloader.is_downloading("123456789"));
        
        downloader.mark_downloaded("123456789");
        assert!(!downloader.is_downloading("123456789"));
    }

    #[test]
    fn test_mark_downloading_multiple() {
        let mut downloader = Downloader::new(None);
        
        downloader.mark_downloading("111111111".to_string());
        downloader.mark_downloading("222222222".to_string());
        downloader.mark_downloading("333333333".to_string());
        
        assert!(downloader.is_downloading("111111111"));
        assert!(downloader.is_downloading("222222222"));
        assert!(downloader.is_downloading("333333333"));
        
        downloader.mark_downloaded("222222222");
        assert!(downloader.is_downloading("111111111"));
        assert!(!downloader.is_downloading("222222222"));
        assert!(downloader.is_downloading("333333333"));
    }

    #[test]
    fn test_downloader_paths() {
        let temp_dir = TempDir::new().unwrap();
        let steamcmd_path = temp_dir.path().join("steamcmd");
        
        let downloader = Downloader::new(Some(steamcmd_path.clone()));
        
        assert_eq!(downloader.steamcmd_path, steamcmd_path);
        assert_eq!(
            downloader.download_path,
            steamcmd_path.join("steamapps").join("workshop").join("content").join("294100")
        );
    }
}
