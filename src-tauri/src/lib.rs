pub mod models;
pub mod services;
pub mod commands;
pub mod utils;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    use std::sync::Arc;

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .manage(
            crate::services::queue_manager::QueueManager::new()
                .expect("无法初始化队列管理器"),
        )
        .invoke_handler(tauri::generate_handler![
            crate::commands::get_queue,
            crate::commands::add_to_queue,
            crate::commands::remove_from_queue,
            crate::commands::clear_queue,
            crate::commands::get_history,
            crate::commands::clear_history,
            crate::commands::get_config,
            crate::commands::save_config,
            crate::commands::start_upload,
            crate::commands::pause_upload,
            crate::commands::retry_upload,
            crate::commands::test_alist_connection,
            crate::commands::get_file_info,
            crate::commands::get_data_path,
            crate::commands::check_health,
            crate::commands::alist_login,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
