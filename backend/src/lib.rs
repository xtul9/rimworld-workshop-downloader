pub mod core;
pub mod services;
pub mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Fix Wayland protocol error (Error 71)
    // See: https://github.com/tauri-apps/tauri/issues/10702
    if std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    
    // Set GDK_BACKEND for Wayland if not already set
    if std::env::var("GDK_BACKEND").unwrap_or_default().is_empty() 
        && std::env::var("XDG_SESSION_TYPE").unwrap_or_default() == "wayland" {
        std::env::set_var("GDK_BACKEND", "wayland");
    }
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            commands::query_mods,
            commands::list_installed_mods,
            commands::update_mod_details,
            commands::update_mods,
            commands::check_backup,
            commands::check_backups,
            commands::restore_backup,
            commands::restore_backups,
            commands::ignore_update,
            commands::undo_ignore_update,
            commands::check_ignored_updates,
            commands::get_file_details,
            commands::get_file_details_batch,
            commands::is_collection,
            commands::is_collection_batch,
            commands::get_collection_details,
            commands::get_collection_details_batch,
            commands::download_mod,
        ])
        .setup(|_app| {
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
