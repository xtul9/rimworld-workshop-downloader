use std::path::PathBuf;
use std::fs;
use std::time::Duration;
use tokio::time::sleep;
use tokio::process::Command;
use tokio::sync::mpsc;
use futures;
use tauri::{AppHandle, Emitter};
use notify::{Watcher, RecommendedWatcher, RecursiveMode, Event};
use std::sync::{Arc, Mutex};

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
        Self::find_steamcmd_executable_static(&self.steamcmd_path).await
    }
    
    /// Static version of find_steamcmd_executable for use in spawned tasks
    async fn find_steamcmd_executable_static(steamcmd_path: &PathBuf) -> Result<PathBuf, String> {
        // Try to find in application resources first
        if let Some(resource_path) = Self::find_steamcmd_from_resources_static(steamcmd_path).await? {
            return Ok(resource_path);
        }

        // Try local path
        let steamcmd_exe = if cfg!(target_os = "windows") {
            "steamcmd.exe"
        } else {
            "steamcmd"
        };
        
        let local_path = steamcmd_path.join(steamcmd_exe);
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
    
    /// Static version of find_steamcmd_from_resources for use in spawned tasks
    async fn find_steamcmd_from_resources_static(steamcmd_path: &PathBuf) -> Result<Option<PathBuf>, String> {
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
            steamcmd_path.join(steamcmd_exe),
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
    /// For small batches (<=4 mods), uses single instance. For larger batches, uses up to max_instances parallel instances.
    /// If mod_sizes is provided, mods are balanced by size across instances.
    /// Returns a receiver channel that yields mods as they are downloaded
    pub async fn download_mods(&mut self, mod_ids: &[String], app: Option<&AppHandle>, max_instances: Option<usize>) -> Result<mpsc::Receiver<Result<DownloadedMod, String>>, String> {
        self.download_mods_with_sizes(mod_ids, None, app, max_instances).await
    }

    /// Download mods with optional size information for load balancing
    /// Returns a receiver channel that yields mods as they are downloaded
    /// The download process runs in the background
    pub async fn download_mods_with_sizes(
        &mut self,
        mod_ids: &[String],
        mod_sizes: Option<&std::collections::HashMap<String, u64>>,
        app: Option<&AppHandle>,
        max_instances: Option<usize>,
    ) -> Result<mpsc::Receiver<Result<DownloadedMod, String>>, String> {
        const MAX_RETRIES: u32 = 6;
        const DEFAULT_MAX_INSTANCES: usize = 1;
        let max_instances = max_instances.unwrap_or(DEFAULT_MAX_INSTANCES);
        let (tx, rx) = mpsc::channel(100); // Buffer up to 100 mods
        
        // Clone necessary data for background task
        let mod_ids_clone = mod_ids.to_vec();
        let mod_sizes_clone = mod_sizes.cloned();
        let app_clone = app.cloned();
        let steamcmd_path = self.steamcmd_path.clone();
        let download_path = self.download_path.clone();
        let tx_clone = tx.clone();
        let max_instances_clone = max_instances;
        
        // Spawn background task to handle downloads
        // This allows the function to return the channel immediately
        tokio::spawn(async move {
            let mut remaining_mod_ids = mod_ids_clone;
            let mut remaining_mod_sizes = mod_sizes_clone;
            let mut retry_count = 0;
            
            while !remaining_mod_ids.is_empty() && retry_count <= MAX_RETRIES {
                if retry_count > 0 {
                    eprintln!("[Downloader] Retry attempt {}: {} mod(s) remaining (attempt {}/{})", 
                        retry_count, remaining_mod_ids.len(), retry_count, MAX_RETRIES);
                    
                    // Emit retry-queued events for remaining mods
                    if let Some(app_handle) = &app_clone {
                        for mod_id in &remaining_mod_ids {
                            let _ = app_handle.emit("mod-state", serde_json::json!({
                                "modId": mod_id,
                                "state": "retry-queued",
                                "retryAttempt": retry_count,
                                "maxRetries": MAX_RETRIES
                            }));
                        }
                    }
                    
                    // Exponential backoff: 1s, 2s, 4s, 8s, 16s, 32s
                    let backoff_seconds = 2_u64.pow(retry_count - 1);
                    eprintln!("[Downloader] Waiting {} seconds before retry...", backoff_seconds);
                    sleep(Duration::from_secs(backoff_seconds)).await;
                }
            
            // Track which mods will be retried (before attempt) to avoid showing "failed" state
            let mods_to_retry: std::collections::HashSet<String> = if retry_count < MAX_RETRIES {
                remaining_mod_ids.iter().cloned().collect()
            } else {
                std::collections::HashSet::new()
            };
            
            // Attempt download (pass mods_to_retry only if we're in retry loop)
            let mods_to_retry_for_attempt = if retry_count > 0 {
                Some(mods_to_retry.clone())
            } else {
                None
            };
            
            let attempt_result = Self::download_mods_single_attempt_static(
                &steamcmd_path,
                &download_path,
                &remaining_mod_ids,
                remaining_mod_sizes.as_ref(),
                app_clone.as_ref(),
                mods_to_retry_for_attempt.as_ref(),
                Some(tx_clone.clone()),
                max_instances_clone,
            ).await;
            
            match attempt_result {
                Ok((downloaded_mods, _failed_mod_ids)) => {
                    // Mods were already sent to channel in wait_for_mod_download_static
                    // We just track which mods were successfully downloaded for retry logic
                    let downloaded_mod_ids: std::collections::HashSet<String> = downloaded_mods
                        .iter()
                        .map(|m| m.mod_id.clone())
                        .collect();
                    
                    // Check which mods still failed (mods that weren't successfully downloaded)
                    remaining_mod_ids = remaining_mod_ids
                        .into_iter()
                        .filter(|id| !downloaded_mod_ids.contains(id))
                        .collect();
                    
                    // Filter mod_sizes to only include remaining mods
                    if let Some(ref sizes) = remaining_mod_sizes {
                        remaining_mod_sizes = Some(
                            sizes.iter()
                                .filter(|(id, _)| remaining_mod_ids.contains(*id))
                                .map(|(id, size)| (id.clone(), *size))
                                .collect()
                        );
                    }
                    
                    // If all mods downloaded, we're done
                    if remaining_mod_ids.is_empty() {
                        break;
                    }
                    
                    // Emit retry-queued IMMEDIATELY for mods that will be retried
                    // This must happen BEFORE any error handling to avoid showing "failed" state
                    if retry_count < MAX_RETRIES {
                        if let Some(app_handle) = &app_clone {
                            for mod_id in &remaining_mod_ids {
                                let _ = app_handle.emit("mod-state", serde_json::json!({
                                    "modId": mod_id,
                                    "state": "retry-queued",
                                    "retryAttempt": retry_count + 1,
                                    "maxRetries": MAX_RETRIES
                                }));
                            }
                        }
                    } else {
                        // All retries exhausted - emit failed state for remaining mods
                        if let Some(app_handle) = &app_clone {
                            for mod_id in &remaining_mod_ids {
                                let _ = app_handle.emit("mod-state", serde_json::json!({
                                    "modId": mod_id,
                                    "state": "failed",
                                    "error": format!("Download failed after {} attempts", MAX_RETRIES)
                                }));
                            }
                        }
                        // Send errors to channel for final failures
                        for _failed_mod_id in &remaining_mod_ids {
                            let _ = tx_clone.send(Err(format!("Download failed after {} attempts", MAX_RETRIES))).await;
                        }
                    }
                    
                    retry_count += 1;
                }
                Err(e) => {
                    eprintln!("[Downloader] Download attempt {} failed: {}", retry_count + 1, e);
                    retry_count += 1;
                    
                    // If we've exceeded max retries, send remaining mods as errors and close channel
                    if retry_count > MAX_RETRIES {
                        // Send remaining mods as errors
                        for _mod_id in &remaining_mod_ids {
                            let _ = tx_clone.send(Err(format!("Download failed after {} attempts", MAX_RETRIES))).await;
                        }
                        
                        if remaining_mod_ids.is_empty() {
                            eprintln!("[Downloader] All mod downloads failed after {} attempts. Last error: {}", 
                                MAX_RETRIES, e);
                        } else {
                            eprintln!("[Downloader] Some mod downloads failed after {} attempts. Failed mods: {}. Last error: {}", 
                                MAX_RETRIES, remaining_mod_ids.join(", "), e);
                        }
                        // Close channel and exit task
                        drop(tx_clone);
                        return;
                    }
                }
            } // Close match attempt_result
            } // Close while loop
            
            // If we still have remaining mods after max retries, they should already be marked as failed
            // in the match block above, so we just log here
            if !remaining_mod_ids.is_empty() {
                eprintln!("[Downloader] Max retries ({}) exceeded for {} mod(s): {}", 
                    MAX_RETRIES, remaining_mod_ids.len(), remaining_mod_ids.join(", "));
            }
            
            // Close channel to signal completion
            drop(tx_clone);
        }); // Close tokio::spawn
        
        // Return channel immediately - downloads happen in background
        Ok(rx)
    }

    /// Single download attempt without retry logic (static version for use in spawned tasks)
    /// Returns tuple of (downloaded_mods, failed_mod_ids)
    async fn download_mods_single_attempt_static(
        steamcmd_path: &PathBuf,
        download_path: &PathBuf,
        mod_ids: &[String],
        mod_sizes: Option<&std::collections::HashMap<String, u64>>,
        app: Option<&AppHandle>,
        mods_to_retry: Option<&std::collections::HashSet<String>>,
        _tx: Option<mpsc::Sender<Result<DownloadedMod, String>>>,
        max_instances: usize,
    ) -> Result<(Vec<DownloadedMod>, Vec<String>), String> {
        // Convert mods_to_retry to owned Option for passing to download_mods_batch
        let mods_to_retry_owned = mods_to_retry.map(|set| set.clone());
        if mod_ids.is_empty() {
            return Ok((vec![], vec![]));
        }

        // Delete appworkshop file if it exists
        let appworkshop_path = steamcmd_path
            .join("steamapps")
            .join("workshop")
            .join("appworkshop_294100.acf");
        let _ = fs::remove_file(&appworkshop_path);

        // Ensure download directory exists
        fs::create_dir_all(download_path)
            .map_err(|e| format!("Failed to create download directory: {}", e))?;

        // Calculate number of instances: use as many as possible (up to max_instances)
        // For 3+ mods, always use parallel instances to maximize throughput
        let num_instances = std::cmp::min(mod_ids.len(), max_instances);
        
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
        let steamcmd_executable = Self::find_steamcmd_executable_static(steamcmd_path).await?;
        
        for (batch_idx, batch) in batches.into_iter().enumerate() {
            if batch.is_empty() {
                continue;
            }
            
            let steamcmd_path_clone = steamcmd_path.clone();
            let download_path_clone = download_path.clone();
            
            let mods_to_retry_for_batch = mods_to_retry_owned.clone();
            let tx_for_batch = _tx.clone();
            let future = Self::download_mods_batch(
                steamcmd_executable.clone(),
                steamcmd_path_clone,
                download_path_clone,
                batch,
                batch_idx,
                app.cloned(),
                mods_to_retry_for_batch,
                tx_for_batch,
            );
            batch_futures.push(future);
        }
        
        // Wait for all batches to complete in parallel
        let batch_results = futures::future::join_all(batch_futures).await;
        let mut all_downloaded_mods = Vec::new();
        let mut all_failed_mod_ids = Vec::new();
        let mut success_count = 0;
        let mut failure_count = 0;
        let mut failed_batches: Vec<(usize, Vec<String>, String)> = Vec::new();
        
        for (batch_idx, result) in batch_results.into_iter().enumerate() {
            match result {
                Ok((mods, failed_ids)) => {
                    let mods_count = mods.len();
                    success_count += 1;
                    all_downloaded_mods.extend(mods);
                    all_failed_mod_ids.extend(failed_ids);
                    eprintln!("[Downloader] Instance {}: completed successfully ({} mod(s))", batch_idx, mods_count);
                }
                Err(e) => {
                    failure_count += 1;
                    eprintln!("[Downloader] Instance {}: failed - {}", batch_idx, e);
                    // Store failed batch info for potential retry
                    // Note: We don't have access to mod_ids here, so we'll handle retry differently
                    failed_batches.push((batch_idx, Vec::new(), e));
                }
            }
        }
        
        // If we have failures and some mods were requested, check which ones failed
        if failure_count > 0 && !all_downloaded_mods.is_empty() {
            let downloaded_mod_ids: std::collections::HashSet<String> = all_downloaded_mods
                .iter()
                .map(|m| m.mod_id.clone())
                .collect();
            let failed_mod_ids: Vec<String> = mod_ids
                .iter()
                .filter(|id| !downloaded_mod_ids.contains(*id))
                .cloned()
                .collect();
            
            if !failed_mod_ids.is_empty() {
                eprintln!("[Downloader] Warning: {} mod(s) failed to download: {}", 
                    failed_mod_ids.len(), failed_mod_ids.join(", "));
            }
        }
        
        eprintln!("[Downloader] All instances completed: {} succeeded, {} failed, {} total mod(s) downloaded", success_count, failure_count, all_downloaded_mods.len());
        
        // If all downloads failed, return error
        if all_downloaded_mods.is_empty() && !mod_ids.is_empty() {
            return Err(format!("All mod downloads failed. Check SteamCMD logs and network connection."));
        }
        
        // Return tuple of (downloaded_mods, failed_mod_ids)
        Ok((all_downloaded_mods, all_failed_mod_ids))
    }

    /// Download a batch of mods using a single SteamCMD instance (static version for parallel execution)
    /// Sends mods to channel immediately as they are downloaded
    /// Returns tuple of (downloaded_mods, failed_mod_ids) for tracking purposes
    async fn download_mods_batch(
        steamcmd_executable: PathBuf,
        steamcmd_path: PathBuf,
        download_path: PathBuf,
        mod_ids: Vec<String>,
        batch_idx: usize,
        app: Option<AppHandle>,
        mods_to_retry: Option<std::collections::HashSet<String>>,
        tx: Option<mpsc::Sender<Result<DownloadedMod, String>>>,
    ) -> Result<(Vec<DownloadedMod>, Vec<String>), String> {
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
        
        // Emit queued events for all mods as they are added to the script
        if let Some(app_handle) = &app {
            for mod_id in &mod_ids {
                let _ = app_handle.emit("mod-state", serde_json::json!({
                    "modId": mod_id,
                    "state": "queued"
                }));
            }
        }
        
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

        // Track failed mods detected from SteamCMD output
        let failed_mods_tracker: Arc<Mutex<std::collections::HashSet<String>>> = Arc::new(Mutex::new(std::collections::HashSet::new()));
        
        // Start watching folders before starting download
        // Each promise will detect mods when downloaded, but won't send to channel yet
        // We'll check failed_mods_tracker before sending to channel
        let mut download_promises = Vec::new();
        for mod_id in &mod_ids {
            let mod_download_path = download_path_absolute.join(mod_id.clone());
            let mod_id_clone = mod_id.clone();
            let app_clone = app.clone();
            let tx_clone = tx.clone();
            let failed_mods_tracker_clone = failed_mods_tracker.clone();
            download_promises.push(Self::wait_for_mod_download_static(
                mod_download_path, 
                mod_id_clone, 
                app_clone,
                tx_clone,
                Some(failed_mods_tracker_clone), // Pass failed_mods_tracker
            ));
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

        // failed_mods_tracker was already created above, now clone for stdout/stderr tasks
        let failed_mods_stdout = failed_mods_tracker.clone();
        let failed_mods_stderr = failed_mods_tracker.clone();
        
        // Track which mods will be retried (to avoid showing "failed" state)
        // Clone for each task separately
        let mods_to_retry_stdout = mods_to_retry.as_ref().map(|set| set.clone());
        let mods_to_retry_stderr = mods_to_retry.as_ref().map(|set| set.clone());
        
        // Parse SteamCMD output to detect mod states
        let stdout = steamcmd_process.stdout.take();
        let stderr = steamcmd_process.stderr.take();
        
        // Clone for each task
        let mod_ids_stdout = mod_ids.clone();
        let mod_ids_stderr = mod_ids.clone();
        let app_stdout = app.clone();
        let app_stderr = app.clone();
        
        let _stdout_task = if let Some(stdout) = stdout {
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    // Parse SteamCMD output to detect mod states
                    Self::parse_steamcmd_output(&line, &mod_ids_stdout, app_stdout.as_ref(), Some(&failed_mods_stdout), mods_to_retry_stdout.as_ref());
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
                while let Ok(Some(line)) = lines.next_line().await {
                    // Parse SteamCMD output to detect mod states
                    Self::parse_steamcmd_output(&line, &mod_ids_stderr, app_stderr.as_ref(), Some(&failed_mods_stderr), mods_to_retry_stderr.as_ref());
                }
            })
        } else {
            tokio::spawn(async {})
        };

        // Wait for SteamCMD to start
        sleep(Duration::from_secs(2)).await;

        // Wait for SteamCMD to exit and check exit status
        let status = steamcmd_process.wait().await
            .map_err(|e| format!("Failed to wait for SteamCMD: {}", e))?;

        // Check if SteamCMD exited successfully
        if !status.success() {
            let exit_code = status.code().unwrap_or(-1);
            eprintln!("[Downloader] Instance {}: SteamCMD exited with error code: {}", batch_idx, exit_code);
            
            // Clean up script file
            let _ = fs::remove_file(&script_path);
            
            // Check if any mods were partially downloaded
            let mut partial_mods = Vec::new();
            for mod_id in &mod_ids {
                let mod_download_path = download_path_absolute.join(mod_id.clone());
                if Self::is_mod_partially_downloaded(&mod_download_path) {
                    partial_mods.push(mod_id.clone());
                }
            }
            
            if !partial_mods.is_empty() {
                return Err(format!(
                    "SteamCMD failed (exit code: {}) but detected partial downloads for mod(s): {}. These may be incomplete.",
                    exit_code,
                    partial_mods.join(", ")
                ));
            } else {
                return Err(format!("SteamCMD failed with exit code: {}. No mods were downloaded.", exit_code));
            }
        }

        // Wait a bit for file system operations
        sleep(Duration::from_secs(1)).await;

        // Wait for all mod downloads to be detected in parallel
        // Note: Each promise sends mods to channel immediately when downloaded,
        // so we're just waiting here to collect results for tracking/failure reporting
        let download_results = futures::future::join_all(download_promises).await;
        let mut downloaded_mods = Vec::new();
        let mut failed_mods = Vec::new();
        
        // Get list of mods that failed according to SteamCMD output
        let steamcmd_failed_mods: std::collections::HashSet<String> = {
            let failed = failed_mods_tracker.lock().unwrap();
            failed.clone()
        };
        
        for (idx, result) in download_results.into_iter().enumerate() {
            let mod_id = &mod_ids[idx];
            
            // Skip mods that SteamCMD reported as failed
            if steamcmd_failed_mods.contains(mod_id) {
                eprintln!("[Downloader] Instance {}: Mod {} failed according to SteamCMD output", batch_idx, mod_id);
                failed_mods.push(mod_id.clone());
                // Send error to channel if tx is available
                if let Some(ref tx_ref) = tx {
                    let _ = tx_ref.send(Err(format!("Download failed for mod {}", mod_id))).await;
                }
                continue;
            }
            
            match result {
                Ok(Some(mod_info)) => {
                    // Verify download completeness before adding
                    if Self::verify_mod_download_complete(&mod_info.mod_path) {
                        downloaded_mods.push(mod_info.clone());
                        // Mod was already sent to channel in wait_for_mod_download_static
                        // We just keep it here for tracking
                    } else {
                        eprintln!("[Downloader] Instance {}: Mod {} detected but download appears incomplete", batch_idx, mod_id);
                        failed_mods.push(mod_id.clone());
                        // Send error to channel if tx is available
                        if let Some(ref tx_ref) = tx {
                            let _ = tx_ref.send(Err(format!("Download incomplete for mod {}", mod_id))).await;
                        }
                    }
                }
                Ok(None) => {
                    eprintln!("[Downloader] Instance {}: Mod {} download timeout or not detected", batch_idx, mod_id);
                    failed_mods.push(mod_id.clone());
                    // Send error to channel if tx is available
                    if let Some(ref tx_ref) = tx {
                        let _ = tx_ref.send(Err(format!("Download timeout for mod {}", mod_id))).await;
                    }
                }
                Err(e) => {
                    eprintln!("[Downloader] Instance {}: Mod {} download error: {}", batch_idx, mod_id, e);
                    failed_mods.push(mod_id.clone());
                    // Send error to channel if tx is available
                    if let Some(ref tx_ref) = tx {
                        let _ = tx_ref.send(Err(format!("Download error for mod {}: {}", mod_id, e))).await;
                    }
                }
            }
        }

        // Clean up script file
        let _ = fs::remove_file(&script_path);

        // If some mods failed, return error with details
        if !failed_mods.is_empty() {
            if downloaded_mods.is_empty() {
                return Err(format!("All mod downloads failed. Failed mods: {}", failed_mods.join(", ")));
            } else {
                eprintln!("[Downloader] Instance {}: Partial success - {} mod(s) downloaded, {} failed: {}", 
                    batch_idx, downloaded_mods.len(), failed_mods.len(), failed_mods.join(", "));
                // Still return success with downloaded mods, but log the failures
            }
        }

        // Return tuple of (downloaded_mods, failed_mod_ids)
        Ok((downloaded_mods, failed_mods))
    }

    /// Wait for a mod to be downloaded by watching the download folder (static version for Send)
    /// Sends mod to channel immediately when downloaded (if tx is provided), but only if not in failed_mods_tracker
    async fn wait_for_mod_download_static(
        mod_download_path: PathBuf,
        mod_id: String,
        app: Option<AppHandle>,
        tx: Option<mpsc::Sender<Result<DownloadedMod, String>>>,
        failed_mods_tracker: Option<Arc<Mutex<std::collections::HashSet<String>>>>,
    ) -> Result<Option<DownloadedMod>, String> {
        let timeout = Duration::from_secs(600); // 10 minutes timeout
        let start_time = std::time::Instant::now();
        
        // First, check if mod is already downloaded (race condition protection)
        if let Ok(metadata) = fs::metadata(&mod_download_path) {
            if metadata.is_dir() {
                if let Ok(entries) = fs::read_dir(&mod_download_path) {
                    if entries.take(1).count() > 0 {
                        // Check if mod is in failed_mods_tracker before sending to channel
                        let should_send = if let Some(ref tracker) = failed_mods_tracker {
                            let failed = tracker.lock().unwrap();
                            !failed.contains(&mod_id)
                        } else {
                            true // If no tracker, send anyway
                        };
                        
                        let result = Self::create_downloaded_mod_result(mod_download_path, mod_id.clone(), app);
                        // Send to channel immediately if available and not failed
                        if should_send {
                            if let Some(ref tx_ref) = tx {
                                if let Ok(Some(mod_info)) = &result {
                                    let _ = tx_ref.send(Ok(mod_info.clone())).await;
                                }
                            }
                            return result;
                        } else {
                            eprintln!("[Downloader] Mod {} detected but SteamCMD reported failure - not sending to channel", mod_id);
                            return Ok(None);
                        }
                    }
                }
            }
        }
        
        // Get parent directory to watch (the workshop content folder)
        let watch_path = mod_download_path.parent()
            .ok_or_else(|| "Cannot get parent directory for watching".to_string())?;
        
        // Create channel for file system events (use std::sync::mpsc for notify compatibility)
        let (tx_fs, rx) = std::sync::mpsc::channel();
        let rx_shared = Arc::new(Mutex::new(rx));
        
        // Create watcher with minimal delay for faster detection
        let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                let _ = tx_fs.send(event);
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
        let tx_mod_channel = tx.clone();
        let failed_mods_tracker_clone = failed_mods_tracker.clone();
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
                                            // Check if mod is in failed_mods_tracker before sending to channel
                                            let should_send = if let Some(ref tracker) = failed_mods_tracker_clone {
                                                let failed = tracker.lock().unwrap();
                                                !failed.contains(&mod_id_clone)
                                            } else {
                                                true // If no tracker, send anyway
                                            };
                                            
                                            let result = Self::create_downloaded_mod_result(
                                                mod_download_path_clone, 
                                                mod_id_clone.clone(), 
                                                app_clone
                                            );
                                            // Send to channel immediately if available and not failed
                                            if should_send {
                                                if let Some(ref tx_ref) = tx_mod_channel {
                                                    if let Ok(Some(mod_info)) = &result {
                                                        let _ = tx_ref.send(Ok(mod_info.clone())).await;
                                                    }
                                                }
                                                return result;
                                            } else {
                                                eprintln!("[Downloader] Mod {} detected but SteamCMD reported failure - not sending to channel", mod_id_clone);
                                                return Ok(None);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(Err(std::sync::mpsc::RecvTimeoutError::Timeout)) => {
                        // Timeout - check if mod exists anyway (fallback polling)
                        if let Ok(metadata) = fs::metadata(&mod_download_path_clone) {
                            if metadata.is_dir() {
                                if let Ok(entries) = fs::read_dir(&mod_download_path_clone) {
                                    if entries.take(1).count() > 0 {
                                        // Check if mod is in failed_mods_tracker before sending to channel
                                        let should_send = if let Some(ref tracker) = failed_mods_tracker_clone {
                                            let failed = tracker.lock().unwrap();
                                            !failed.contains(&mod_id_clone)
                                        } else {
                                            true // If no tracker, send anyway
                                        };
                                        
                                        let result = Self::create_downloaded_mod_result(
                                            mod_download_path_clone, 
                                            mod_id_clone.clone(), 
                                            app_clone
                                        );
                                        // Send to channel immediately if available and not failed
                                        if should_send {
                                            if let Some(ref tx_ref) = tx_mod_channel {
                                                if let Ok(Some(mod_info)) = &result {
                                                    let _ = tx_ref.send(Ok(mod_info.clone())).await;
                                                }
                                            }
                                            return result;
                                        } else {
                                            eprintln!("[Downloader] Mod {} detected but SteamCMD reported failure - not sending to channel", mod_id_clone);
                                            return Ok(None);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Ok(Err(std::sync::mpsc::RecvTimeoutError::Disconnected)) => {
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
                            // Check if mod is in failed_mods_tracker before sending to channel
                            let should_send = if let Some(ref tracker) = failed_mods_tracker_clone {
                                let failed = tracker.lock().unwrap();
                                !failed.contains(&mod_id_clone)
                            } else {
                                true // If no tracker, send anyway
                            };
                            
                            let result = Self::create_downloaded_mod_result(
                                mod_download_path_clone, 
                                mod_id_clone.clone(), 
                                app_clone
                            );
                            // Send to channel immediately if available and not failed
                            if should_send {
                                if let Some(ref tx_ref) = tx_mod_channel {
                                    if let Ok(Some(mod_info)) = &result {
                                        let _ = tx_ref.send(Ok(mod_info.clone())).await;
                                    }
                                }
                                return result;
                            } else {
                                eprintln!("[Downloader] Mod {} detected but SteamCMD reported failure - not sending to channel", mod_id_clone);
                                return Ok(None);
                            }
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
                                let result = Self::create_downloaded_mod_result(mod_download_path, mod_id.clone(), app);
                                // Send to channel immediately if available
                                if let Some(ref tx_ref) = tx {
                                    if let Ok(Some(mod_info)) = &result {
                                        let _ = tx_ref.send(Ok(mod_info.clone())).await;
                                    }
                                }
                                return result;
                            }
                        }
                    }
                }
                Ok(None)
            }
        }
    }
    
    /// Check if a mod appears to be partially downloaded (folder exists but may be incomplete)
    /// TODO: figure out a way to check integrity of the download compared to Steam Workshop
    fn is_mod_partially_downloaded(mod_path: &PathBuf) -> bool {
        if !mod_path.exists() || !mod_path.is_dir() {
            return false;
        }
        
        // Check if folder has any content
        if let Ok(entries) = fs::read_dir(mod_path) {
            return entries.take(1).count() > 0;
        }
        
        false
    }

    /// Verify that a mod download is complete by checking for essential files
    fn verify_mod_download_complete(mod_path: &PathBuf) -> bool {
        // Check if mod folder exists and is a directory
        if !mod_path.exists() || !mod_path.is_dir() {
            eprintln!("[Downloader] Mod path does not exist or is not a directory: {:?}", mod_path);
            return false;
        }
        
        // Check if folder has any content
        let has_content = if let Ok(entries) = fs::read_dir(mod_path) {
            entries.take(1).count() > 0
        } else {
            false
        };
        
        if !has_content {
            eprintln!("[Downloader] Mod folder is empty: {:?}", mod_path);
            return false;
        }
        
        // Check for About folder (essential for RimWorld mods)
        let about_path = mod_path.join("About");
        if !about_path.exists() || !about_path.is_dir() {
            eprintln!("[Downloader] Mod missing About folder: {:?}", mod_path);
            return false;
        }
        
        // Check for PublishedFileId.txt (should exist for Workshop mods)
        // Note: We create this file automatically if missing, so this is just a sanity check
        let published_file_id_path = about_path.join("PublishedFileId.txt");
        if !published_file_id_path.exists() {
            eprintln!("[Downloader] Warning: Mod missing PublishedFileId.txt (will be created automatically): {:?}", mod_path);
            // Don't fail here - we create this file automatically in mod_manager
        }
        
        // Additional check: verify folder has reasonable size (at least 1KB)
        // This helps catch cases where only empty folders were created
        if let Ok(metadata) = fs::metadata(mod_path) {
            // For directories, we can't easily check total size without recursion
            // But we can check if it's a valid directory
            if !metadata.is_dir() {
                eprintln!("[Downloader] Mod path is not a directory: {:?}", mod_path);
                return false;
            }
        }
        
        true
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

    /// Parse SteamCMD output to detect mod states and emit events
    fn parse_steamcmd_output(
        line: &str, 
        mod_ids: &[String], 
        app: Option<&AppHandle>,
        failed_mods_tracker: Option<&Arc<Mutex<std::collections::HashSet<String>>>>,
        mods_to_retry: Option<&std::collections::HashSet<String>>,
    ) {
        let line_trimmed = line.trim();
        let line_lower = line_trimmed.to_lowercase();
        
        // Log SteamCMD output for debugging (can be removed later if too verbose)
        if line_trimmed.len() > 0 && !line_lower.contains("steam>") && !line_lower.contains("loading") {
            eprintln!("[SteamCMD Output] {}", line_trimmed);
        }
        
        // Check each mod ID in the batch
        for mod_id in mod_ids {
            // Check if this line mentions the mod ID
            if !line_lower.contains(mod_id) {
                continue;
            }
            
            // Detect download errors FIRST - before other states
            // Pattern: "ERROR! Download item <mod_id> failed (Failure)"
            if line_lower.contains("error") && 
               line_lower.contains("download") &&
               line_lower.contains("failed") &&
               line_lower.contains(mod_id) {
                // Track failed mod
                if let Some(tracker) = failed_mods_tracker {
                    let mut failed = tracker.lock().unwrap();
                    failed.insert(mod_id.clone());
                }
                
                // Only emit "failed" if this mod won't be retried
                // If it will be retried, "retry-queued" will be emitted in retry logic
                let will_retry = mods_to_retry.map(|set| set.contains(mod_id)).unwrap_or(false);
                
                if let Some(app_handle) = app {
                    if !will_retry {
                        eprintln!("[SteamCMD Parser] Mod {} detected as failed (no retry)", mod_id);
                        let _ = app_handle.emit("mod-state", serde_json::json!({
                            "modId": mod_id,
                            "state": "failed",
                            "error": "SteamCMD reported download failure"
                        }));
                    } else {
                        eprintln!("[SteamCMD Parser] Mod {} detected as failed (will retry, not emitting failed state)", mod_id);
                        // Don't emit "failed" - retry logic will emit "retry-queued"
                    }
                }
                continue; // Don't process other states for failed mods
            }
            
            // Detect downloading state - when SteamCMD actually starts downloading
            // Patterns: "Downloading item <mod_id>", "Downloading Workshop item <mod_id>", etc.
            // But NOT if it's part of "downloaded" or "download failed"
            if line_lower.contains("downloading") && 
               !line_lower.contains("downloaded") &&
               !line_lower.contains("failed") &&
               (line_lower.contains("item") || line_lower.contains("workshop")) &&
               line_lower.contains(mod_id) {
                if let Some(app_handle) = app {
                    eprintln!("[SteamCMD Parser] Mod {} detected as downloading", mod_id);
                    let _ = app_handle.emit("mod-state", serde_json::json!({
                        "modId": mod_id,
                        "state": "downloading"
                    }));
                }
            }
        }
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
