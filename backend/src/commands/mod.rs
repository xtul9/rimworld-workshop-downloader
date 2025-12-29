// Tauri command handlers - API layer

pub mod query_handlers;
pub mod update_handlers;
pub mod backup_handlers;
pub mod ignore_handlers;
pub mod workshop_handlers;
pub mod download_handlers;
pub mod watcher_handlers;
pub mod types;

// Re-export all handlers for easy access
pub use query_handlers::*;
pub use update_handlers::*;
pub use backup_handlers::*;
pub use ignore_handlers::*;
pub use workshop_handlers::*;
pub use download_handlers::*;
pub use watcher_handlers::*;
