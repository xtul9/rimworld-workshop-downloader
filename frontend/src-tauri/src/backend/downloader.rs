use std::path::PathBuf;
use std::fs;
use std::time::Duration;
use tokio::time::sleep;
use tokio::process::Command;

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

    /// Download mods using SteamCMD
    pub async fn download_mods(&mut self, mod_ids: &[String]) -> Result<Vec<DownloadedMod>, String> {
        // Delete appworkshop file if it exists
        let appworkshop_path = self.steamcmd_path
            .join("steamapps")
            .join("workshop")
            .join("appworkshop_294100.acf");
        let _ = fs::remove_file(&appworkshop_path);

        // Ensure download directory exists
        fs::create_dir_all(&self.download_path)
            .map_err(|e| format!("Failed to create download directory: {}", e))?;

        eprintln!("Downloading {} workshop mods with SteamCMD", mod_ids.len());

        // Get absolute path to steamcmd directory
        let steamcmd_path_absolute = if self.steamcmd_path.is_absolute() {
            self.steamcmd_path.clone()
        } else {
            let current_dir = std::env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;
            current_dir.join(&self.steamcmd_path)
        };
        
        // Get absolute path to download directory
        let download_path_absolute = if self.download_path.is_absolute() {
            self.download_path.clone()
        } else {
            let current_dir = std::env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;
            current_dir.join(&self.download_path)
        };

        eprintln!("[Downloader] SteamCMD path (absolute): {:?}", steamcmd_path_absolute);
        eprintln!("[Downloader] Download path (absolute): {:?}", download_path_absolute);

        // Create steamcmd script - use absolute path
        let mut script_lines = vec![
            format!("force_install_dir \"{}\"", steamcmd_path_absolute.to_string_lossy()),
            "login anonymous".to_string(),
        ];
        
        for mod_id in mod_ids {
            script_lines.push(format!("workshop_download_item 294100 {}", mod_id));
        }
        
        script_lines.push("quit".to_string());

        let script_content = script_lines.join("\n") + "\n";
        let script_path = self.steamcmd_path.join("run.txt");
        fs::write(&script_path, script_content)
            .map_err(|e| format!("Failed to write SteamCMD script: {}", e))?;

        // Get absolute path to script file
        let script_path_absolute = if script_path.is_absolute() {
            script_path
        } else {
            // Convert to absolute path
            let current_dir = std::env::current_dir()
                .map_err(|e| format!("Failed to get current directory: {}", e))?;
            current_dir.join(&script_path)
        };

        // Find SteamCMD executable
        let steamcmd_executable = self.find_steamcmd_executable().await?;
        eprintln!("[Downloader] Using SteamCMD executable: {:?}", steamcmd_executable);
        eprintln!("[Downloader] Working directory: {:?}", self.steamcmd_path);
        eprintln!("[Downloader] Download path: {:?}", self.download_path);
        eprintln!("[Downloader] Script path (absolute): {:?}", script_path_absolute);

        // Start watching folders before starting download - use absolute path
        let mut download_promises = Vec::new();
        for mod_id in mod_ids {
            let mod_download_path = download_path_absolute.join(mod_id.clone());
            eprintln!("[Downloader] Will watch for mod {} at {:?}", mod_id, mod_download_path);
            let mod_id_clone = mod_id.clone();
            download_promises.push(Self::wait_for_mod_download_static(mod_download_path, mod_id_clone));
        }

        eprintln!("[Downloader] Starting SteamCMD process...");
        // Start SteamCMD process - use absolute path to script and working directory
        let mut steamcmd_process = Command::new(&steamcmd_executable)
            .arg("+runscript")
            .arg(&script_path_absolute)
            .current_dir(&steamcmd_path_absolute)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn SteamCMD: {}", e))?;

        eprintln!("[Downloader] SteamCMD process started (PID: {:?})", steamcmd_process.id());

        // Read output in background
        let stdout = steamcmd_process.stdout.take();
        let stderr = steamcmd_process.stderr.take();
        
        let stdout_task = if let Some(stdout) = stdout {
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    eprintln!("[SteamCMD stdout] {}", line);
                }
            })
        } else {
            tokio::spawn(async {})
        };

        let stderr_task = if let Some(stderr) = stderr {
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    eprintln!("[SteamCMD stderr] {}", line);
                }
            })
        } else {
            tokio::spawn(async {})
        };

        // Wait for SteamCMD to start and login
        eprintln!("[Downloader] Waiting 3 seconds for SteamCMD to start...");
        sleep(Duration::from_secs(3)).await;

        // Wait for SteamCMD to exit
        eprintln!("[Downloader] Waiting for SteamCMD to exit...");
        let status = steamcmd_process.wait().await
            .map_err(|e| format!("Failed to wait for SteamCMD: {}", e))?;
        
        // Wait a bit for output reading to complete
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        if !status.success() {
            eprintln!("[Downloader] SteamCMD exited with code {:?}", status.code());
        } else {
            eprintln!("[Downloader] SteamCMD exited successfully");
        }
        
        // Cancel output reading tasks
        stdout_task.abort();
        stderr_task.abort();

        // Wait a bit more for file system operations to complete
        sleep(Duration::from_secs(2)).await;

        // Wait for all mod downloads to be detected
        eprintln!("[Downloader] Waiting for {} mod(s) to be detected...", download_promises.len());
        let mut downloaded_mods = Vec::new();
        for promise in download_promises {
            if let Ok(Some(mod_info)) = promise.await {
                downloaded_mods.push(mod_info);
            }
        }
        eprintln!("[Downloader] Detected {} downloaded mod(s)", downloaded_mods.len());

        Ok(downloaded_mods)
    }

    /// Wait for a mod to be downloaded by watching the download folder (static version for Send)
    async fn wait_for_mod_download_static(
        mod_download_path: PathBuf,
        mod_id: String,
    ) -> Result<Option<DownloadedMod>, String> {
        let timeout = Duration::from_secs(300); // 5 minutes timeout
        let start_time = std::time::Instant::now();
        let check_interval = Duration::from_secs(1);
        let mut last_log_time = std::time::Instant::now();
        let log_interval = Duration::from_secs(10); // Log every 10 seconds

        eprintln!("[Downloader] Starting to watch for mod {} at {:?}", mod_id, mod_download_path);

        loop {
            // Check if path exists
            match fs::metadata(&mod_download_path) {
                Ok(metadata) => {
                    if metadata.is_dir() {
                        // Mod folder exists, check if it has content
                        match fs::read_dir(&mod_download_path) {
                            Ok(entries) => {
                                let count = entries.count();
                                if count > 0 {
                                    eprintln!("[Downloader] Mod {} downloaded successfully to {:?} (found {} items)", mod_id, mod_download_path, count);
                                    let folder = mod_download_path.file_name()
                                        .and_then(|n| n.to_str())
                                        .map(|s| s.to_string());
                                    return Ok(Some(DownloadedMod {
                                        mod_id: mod_id.clone(),
                                        mod_path: mod_download_path,
                                        folder,
                                    }));
                                } else {
                                    if last_log_time.elapsed() >= log_interval {
                                        eprintln!("[Downloader] Mod {} folder exists but is empty, waiting... ({:.0}s elapsed)", mod_id, start_time.elapsed().as_secs());
                                        last_log_time = std::time::Instant::now();
                                    }
                                }
                            }
                            Err(e) => {
                                if last_log_time.elapsed() >= log_interval {
                                    eprintln!("[Downloader] Error reading mod {} directory: {} ({:.0}s elapsed)", mod_id, e, start_time.elapsed().as_secs());
                                    last_log_time = std::time::Instant::now();
                                }
                            }
                        }
                    } else {
                        if last_log_time.elapsed() >= log_interval {
                            eprintln!("[Downloader] Mod {} path exists but is not a directory ({:.0}s elapsed)", mod_id, start_time.elapsed().as_secs());
                            last_log_time = std::time::Instant::now();
                        }
                    }
                }
                Err(_) => {
                    // Path doesn't exist yet
                    if last_log_time.elapsed() >= log_interval {
                        eprintln!("[Downloader] Waiting for mod {} to appear at {:?} ({:.0}s elapsed)", mod_id, mod_download_path, start_time.elapsed().as_secs());
                        last_log_time = std::time::Instant::now();
                    }
                }
            }

            // Check timeout
            if start_time.elapsed() > timeout {
                eprintln!("[Downloader] Timeout waiting for mod {} to download at {:?} (waited {:.0}s)", mod_id, mod_download_path, timeout.as_secs());
                return Ok(None);
            }

            sleep(check_interval).await;
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
