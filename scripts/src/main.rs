use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn get_steamcmd_urls() -> Vec<String> {
    let platform = std::env::consts::OS;
    
    let urls = match platform {
        "linux" => {
            vec!["https://steamcdn-a.akamaihd.net/client/installer/steamcmd_linux.tar.gz".to_string()]
        }
        "windows" => {
            vec![
                "https://steamcdn-a.akamaihd.net/client/installer/steamcmd.zip".to_string(),
                "https://steamcdn-a.akamaihd.net/client/installer/steamcmd_win32.zip".to_string(),
            ]
        }
        "macos" => {
            vec!["https://steamcdn-a.akamaihd.net/client/installer/steamcmd_osx.tar.gz".to_string()]
        }
        _ => {
            eprintln!("Unsupported platform: {}", platform);
            std::process::exit(1);
        }
    };
    
    urls
}

fn download_file(url: &str, output_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("Downloading SteamCMD from {}...", url);
    
    let client = reqwest::blocking::Client::builder()
        .user_agent("RimworldWorkshopDownloader/1.0")
        .build()?;
    
    let response = client.get(url).send()?;
    
    if response.status().is_redirection() {
        let redirect_url = response.headers()
            .get("location")
            .and_then(|h| h.to_str().ok())
            .ok_or("Redirect received but no location header")?;
        
        let absolute_url = if redirect_url.starts_with("http") {
            redirect_url.to_string()
        } else {
            format!("{}/{}", url.rsplit('/').next().unwrap_or(""), redirect_url)
        };
        
        return download_file(&absolute_url, output_path);
    }
    
    if !response.status().is_success() {
        return Err(format!("Failed to download: {} {}", response.status(), response.status().canonical_reason().unwrap_or("")).into());
    }
    
    let mut file = fs::File::create(output_path)?;
    io::copy(&mut response.bytes()?.as_ref(), &mut file)?;
    
    Ok(())
}

fn extract_tar_gz(tar_gz_path: &Path, output_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("Extracting SteamCMD...");
    fs::create_dir_all(output_dir)?;
    
    let tar_gz = fs::File::open(tar_gz_path)?;
    let tar = flate2::read::GzDecoder::new(tar_gz);
    let mut archive = tar::Archive::new(tar);
    archive.unpack(output_dir)?;
    
    Ok(())
}

fn extract_zip(zip_path: &Path, output_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    println!("Extracting SteamCMD...");
    fs::create_dir_all(output_dir)?;
    
    let file = fs::File::open(zip_path)?;
    let mut archive = zip::ZipArchive::new(file)?;
    
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = output_dir.join(file.mangled_name());
        
        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            let mut outfile = fs::File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }
    }
    
    Ok(())
}

fn find_steamcmd_executable(bin_dir: &Path, platform: &str) -> Option<PathBuf> {
    let steamcmd_exe = if platform == "windows" { "steamcmd.exe" } else { "steamcmd" };
    
    let possible_paths = if platform == "linux" {
        vec![
            bin_dir.join("linux32").join(steamcmd_exe),
            bin_dir.join("linux64").join(steamcmd_exe),
            bin_dir.join(steamcmd_exe),
            bin_dir.parent().unwrap().join("steamcmd").join(steamcmd_exe),
        ]
    } else if platform == "windows" {
        vec![
            bin_dir.join("steamcmd").join(steamcmd_exe),
            bin_dir.join(steamcmd_exe),
            bin_dir.parent().unwrap().join("steamcmd").join(steamcmd_exe),
        ]
    } else {
        vec![
            bin_dir.join(steamcmd_exe),
            bin_dir.parent().unwrap().join("steamcmd").join(steamcmd_exe),
        ]
    };
    
    possible_paths.into_iter().find(|path| path.exists())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let platform = std::env::consts::OS;
    let urls = get_steamcmd_urls();
    
    // Output to bin/steamcmd in project root (independent of backend directory)
    // Tauri expects: ../../bin/steamcmd/steamcmd (relative to frontend/src-tauri/)
    // When running from scripts/, we need to go up one level to project root
    let current_dir = std::env::current_dir()?;
    let project_root = current_dir.parent().ok_or("Cannot find project root directory")?;
    let bin_dir = project_root.join("bin").join("steamcmd");
    fs::create_dir_all(&bin_dir)?;
    
    let is_zip = urls[0].ends_with(".zip");
    let archive_name = if is_zip { "steamcmd.zip" } else { "steamcmd.tar.gz" };
    let archive_path = bin_dir.join(archive_name);
    
    // Try each URL until one works
    let mut download_success = false;
    let mut last_error = None;
    
    for url in &urls {
        match download_file(url, &archive_path) {
            Ok(_) => {
                download_success = true;
                break;
            }
            Err(e) => {
                eprintln!("Failed to download from {}: {}", url, e);
                last_error = Some(e);
                // Clean up failed download
                let _ = fs::remove_file(&archive_path);
                continue;
            }
        }
    }
    
    if !download_success {
        return Err(format!("Failed to download SteamCMD from all URLs. Last error: {}", 
            last_error.map(|e| e.to_string()).unwrap_or_else(|| "Unknown error".to_string())).into());
    }
    
    // Extract archive
    if is_zip {
        extract_zip(&archive_path, &bin_dir)?;
    } else {
        extract_tar_gz(&archive_path, &bin_dir)?;
    }
    
    // Find steamcmd executable
    let source_path = find_steamcmd_executable(&bin_dir, platform)
        .ok_or_else(|| {
            // List directory contents to help debug
            if let Ok(entries) = fs::read_dir(&bin_dir) {
                eprintln!("Files in {:?}:", bin_dir);
                for entry in entries.flatten() {
                    eprintln!("  {:?}", entry.path());
                }
            }
            format!("SteamCMD executable not found after extraction. Expected: {}", 
                if platform == "windows" { "steamcmd.exe" } else { "steamcmd" })
        })?;
    
    println!("Found SteamCMD at: {:?}", source_path);
    
    let steamcmd_exe = if platform == "windows" { "steamcmd.exe" } else { "steamcmd" };
    let target_path = bin_dir.join(steamcmd_exe);
    
    // Move or copy to target location
    if source_path != target_path {
        if fs::rename(&source_path, &target_path).is_err() {
            println!("Rename failed, trying copy...");
            fs::copy(&source_path, &target_path)?;
            println!("Copied SteamCMD from {:?} to {:?}", source_path, target_path);
        } else {
            println!("Moved SteamCMD from {:?} to {:?}", source_path, target_path);
        }
    }
    
    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&target_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&target_path, perms)?;
        println!("Set executable permissions on {:?}", target_path);
    }
    
    // Clean up archive
    let _ = fs::remove_file(&archive_path);
    
    // Clean up extracted directory if it exists
    let extracted_dir_path = bin_dir.join("steamcmd");
    if extracted_dir_path.exists() && extracted_dir_path.is_dir() && extracted_dir_path != bin_dir {
        let _ = fs::remove_dir_all(&extracted_dir_path);
    }
    
    // Rename to match Tauri externalBin naming convention
    let final_name = if platform == "windows" { "steamcmd.exe" } else { "steamcmd" };
    let final_path = bin_dir.join(final_name);
    
    if target_path != final_path && fs::rename(&target_path, &final_path).is_err() {
        let _ = fs::copy(&target_path, &final_path);
    }
    
    // For Tauri, we need to create copies with target triple suffix for each platform
    let target_triples = match platform {
        "linux" => vec!["x86_64-unknown-linux-gnu"],
        "windows" => vec!["x86_64-pc-windows-msvc"],
        "macos" => vec!["x86_64-apple-darwin", "aarch64-apple-darwin"],
        _ => vec![],
    };
    
    for triple in target_triples {
        let suffix_name = if platform == "windows" {
            format!("steamcmd-{}.exe", triple)
        } else {
            format!("steamcmd-{}", triple)
        };
        let suffix_path = bin_dir.join(&suffix_name);
        
        if let Err(e) = fs::copy(&final_path, &suffix_path) {
            eprintln!("Failed to create {}: {}", suffix_name, e);
        } else {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&suffix_path)?.permissions();
                perms.set_mode(0o755);
                let _ = fs::set_permissions(&suffix_path, perms);
            }
            println!("Created {} for Tauri bundle", suffix_name);
        }
    }
    
    println!("SteamCMD downloaded to {:?}", final_path);
    Ok(())
}

