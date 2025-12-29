// Access check utilities

use std::path::{Path, PathBuf};
use std::fs;
use std::io::Write;
use tauri::{AppHandle, Emitter};
use serde_json;

/// Check if a directory has read and write access
/// Returns Ok(()) if access is available, Err with details if not
pub fn check_directory_access(path: &Path) -> Result<(), AccessError> {
    // Check if path exists
    if !path.exists() {
        return Err(AccessError {
            path: path.to_path_buf(),
            can_read: false,
            can_write: false,
            reason: format!("Directory does not exist: {}", path.display()),
        });
    }

    // Check if it's a directory
    if !path.is_dir() {
        return Err(AccessError {
            path: path.to_path_buf(),
            can_read: false,
            can_write: false,
            reason: format!("Path is not a directory: {}", path.display()),
        });
    }

    // Check read access by trying to read directory entries
    let can_read = match fs::read_dir(path) {
        Ok(_) => true,
        Err(e) => {
            return Err(AccessError {
                path: path.to_path_buf(),
                can_read: false,
                can_write: false,
                reason: format!("Cannot read directory: {}", e),
            });
        }
    };

    // Check write access by trying to create a temporary file
    let test_file = path.join(".access_test_temp_file");
    let can_write = match fs::File::create(&test_file) {
        Ok(mut file) => {
            // Try to write to the file
            let write_result = file.write_all(b"test");
            // Always try to remove the test file
            let _ = fs::remove_file(&test_file);
            write_result.is_ok()
        }
        Err(_) => false,
    };

    if !can_write {
        return Err(AccessError {
            path: path.to_path_buf(),
            can_read,
            can_write: false,
            reason: format!("Cannot write to directory: {}", path.display()),
        });
    }

    Ok(())
}

#[derive(Debug, Clone)]
pub struct AccessError {
    pub path: PathBuf,
    pub can_read: bool,
    pub can_write: bool,
    pub reason: String,
}

impl std::fmt::Display for AccessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Access error for {}: {} (read: {}, write: {})",
            self.path.display(),
            self.reason,
            self.can_read,
            self.can_write
        )
    }
}

impl std::error::Error for AccessError {}

/// Ensure directory has read and write access, emit error event and return error if not
/// This is a convenience function that combines access check with event emission
pub fn ensure_directory_access(
    app: &AppHandle,
    path: &Path,
    path_str: &str,
) -> Result<(), String> {
    match check_directory_access(path) {
        Ok(()) => Ok(()),
        Err(access_error) => {
            let error_payload = serde_json::json!({
                "path": path_str,
                "canRead": access_error.can_read,
                "canWrite": access_error.can_write,
                "reason": access_error.reason,
            });
            let _ = app.emit("no-access-error", error_payload);
            Err(format!("No access to mods directory: {}", access_error))
        }
    }
}

/// Check directory access and warn if write access is missing (but don't fail for read-only operations)
/// Returns Ok(()) if read access is available, emits warning if write access is missing
pub fn check_directory_access_with_warning(
    app: &AppHandle,
    path: &Path,
    path_str: &str,
) -> Result<(), String> {
    match check_directory_access(path) {
        Ok(()) => Ok(()),
        Err(access_error) => {
            // For listing, we only need read access, but we check both to inform the user
            if !access_error.can_read {
                let error_payload = serde_json::json!({
                    "path": path_str,
                    "canRead": false,
                    "canWrite": access_error.can_write,
                    "reason": access_error.reason,
                });
                let _ = app.emit("no-access-error", error_payload);
                return Err(format!("No read access to mods directory: {}", access_error));
            }
            // If we can read but not write, warn the user but allow listing
            if !access_error.can_write {
                let error_payload = serde_json::json!({
                    "path": path_str,
                    "canRead": true,
                    "canWrite": false,
                    "reason": access_error.reason,
                });
                let _ = app.emit("no-access-error", error_payload);
            }
            Ok(())
        }
    }
}

