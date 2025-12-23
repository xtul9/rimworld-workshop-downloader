use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::fs;
use std::time::Duration;
use tokio::time::sleep;

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

    /// Find SteamCMD executable from application resources or PATH
    pub async fn find_steamcmd_executable(&self) -> Result<PathBuf, Box<dyn std::error::Error>> {
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
        if let Ok(output) = Command::new(if cfg!(target_os = "windows") { "where" } else { "which" })
            .arg(steamcmd_exe)
            .output()
        {
            if output.status.success() {
                let path_str = String::from_utf8_lossy(&output.stdout);
                let path = PathBuf::from(path_str.trim().lines().next().unwrap_or(""));
                if path.exists() {
                    return Ok(path);
                }
            }
        }

        Err(format!("SteamCMD not found in resources, at {:?}, or in PATH", local_path).into())
    }

    /// Find SteamCMD from application resources (bundled with app)
    async fn find_steamcmd_from_resources(&self) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
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
        let exe_path = std::env::current_exe()?;
        let exe_dir = exe_path.parent().ok_or("Cannot get executable directory")?;
        
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
    pub async fn download_mods(&mut self, mod_ids: &[String]) -> Result<Vec<DownloadedMod>, Box<dyn std::error::Error>> {
        // Delete appworkshop file if it exists
        let appworkshop_path = self.steamcmd_path
            .join("steamapps")
            .join("workshop")
            .join("appworkshop_294100.acf");
        let _ = fs::remove_file(&appworkshop_path);

        // Ensure download directory exists
        fs::create_dir_all(&self.download_path)?;

        eprintln!("Downloading {} workshop mods with SteamCMD", mod_ids.len());

        // Create steamcmd script
        let mut script_lines = vec![
            format!("force_install_dir \"{}\"", self.steamcmd_path.to_string_lossy()),
            "login anonymous".to_string(),
        ];
        
        for mod_id in mod_ids {
            script_lines.push(format!("workshop_download_item 294100 {}", mod_id));
        }
        
        script_lines.push("quit".to_string());

        let script_content = script_lines.join("\n") + "\n";
        let script_path = self.steamcmd_path.join("run.txt");
        fs::write(&script_path, script_content)?;

        // Find SteamCMD executable
        let steamcmd_executable = self.find_steamcmd_executable().await?;

        // Start watching folders before starting download
        let mut download_promises = Vec::new();
        for mod_id in mod_ids {
            let mod_download_path = self.download_path.join(mod_id.clone());
            let mod_id_clone = mod_id.clone();
            download_promises.push(self.wait_for_mod_download(mod_download_path, mod_id_clone));
        }

        // Start SteamCMD process
        let mut steamcmd_process = Command::new(&steamcmd_executable)
            .arg("+runscript")
            .arg(&script_path)
            .current_dir(&self.steamcmd_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Wait for SteamCMD to start and login
        sleep(Duration::from_secs(3)).await;

        // Wait for SteamCMD to exit
        let output = steamcmd_process.wait_with_output()?;
        
        if !output.status.success() {
            eprintln!("[Downloader] SteamCMD exited with code {:?}", output.status.code());
        } else {
            eprintln!("[Downloader] SteamCMD exited successfully");
        }

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

    /// Wait for a mod to be downloaded by watching the download folder
    async fn wait_for_mod_download(
        &self,
        mod_download_path: PathBuf,
        mod_id: String,
    ) -> Result<Option<DownloadedMod>, Box<dyn std::error::Error>> {
        let timeout = Duration::from_secs(300); // 5 minutes timeout
        let start_time = std::time::Instant::now();
        let check_interval = Duration::from_secs(1);

        loop {
            if let Ok(metadata) = fs::metadata(&mod_download_path) {
                if metadata.is_dir() {
                    // Mod folder exists, check if it has content
                    if let Ok(entries) = fs::read_dir(&mod_download_path) {
                        let count = entries.count();
                        if count > 0 {
                            eprintln!("[Downloader] Mod {} downloaded successfully to {:?}", mod_id, mod_download_path);
                            let folder = mod_download_path.file_name()
                                .and_then(|n| n.to_str())
                                .map(|s| s.to_string());
                            return Ok(Some(DownloadedMod {
                                mod_id: mod_id.clone(),
                                mod_path: mod_download_path,
                                folder,
                            }));
                        } else {
                            eprintln!("[Downloader] Mod {} folder exists but is empty, waiting...", mod_id);
                        }
                    }
                }
            }

            // Check timeout
            if start_time.elapsed() > timeout {
                eprintln!("[Downloader] Timeout waiting for mod {} to download at {:?}", mod_id, mod_download_path);
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
