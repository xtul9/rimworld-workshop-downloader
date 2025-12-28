// Input/output types for commands to reduce parameter count

use serde::{Deserialize, Serialize};
use crate::core::mod_scanner::BaseMod;

/// Input for update_mods command
#[derive(Debug, Clone)]
pub struct UpdateModsInput {
    pub mods: Vec<BaseMod>,
    pub backup_mods: bool,
    pub backup_directory: Option<String>,
}

/// Input for backup operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupInput {
    pub mod_path: String,
    pub backup_directory: Option<String>,
}

/// Input for batch backup operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupBatchInput {
    pub mod_paths: Vec<String>,
    pub backup_directory: Option<String>,
}

/// Input for restore backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreBackupInput {
    pub mod_path: String,
    pub backup_directory: String,
}

/// Input for batch restore backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreBackupBatchInput {
    pub mod_paths: Vec<String>,
    pub backup_directory: String,
}

/// Input for download mod
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadModInput {
    pub mod_id: String,
    pub title: Option<String>,
    pub mods_path: String,
}

