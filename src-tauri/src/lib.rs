pub mod models;
pub mod services;
pub mod commands;
pub mod utils;
use tauri::Manager;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;

fn append_log(file_name: &str, message: &str) {
    use std::fs::{self, OpenOptions};
    use std::io::Write;

    let Some(mut log_dir) = dirs::data_local_dir() else {
        return;
    };

    log_dir.push("alist-uploader");

    if fs::create_dir_all(&log_dir).is_err() {
        return;
    }

    let log_path = log_dir.join(file_name);
    let timestamp = chrono::Local::now().to_rfc3339();

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "[{timestamp}] {message}");
    }
}

fn install_panic_hook() {
    std::panic::set_hook(Box::new(|panic_info| {
        append_log("panic.log", &format!("{panic_info}"));
        append_log("startup.log", &format!("panic: {panic_info}"));
    }));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_panic_hook();
    append_log("startup.log", "application startup begin");
    crate::utils::log::log("application startup begin; build_marker=state-free-login-config-v2");

    let queue_manager = match crate::services::queue_manager::QueueManager::new() {
        Ok(manager) => manager,
        Err(error) => {
            append_log("startup.log", &format!("failed to initialize queue manager: {error:?}"));
            panic!("无法初始化队列管理器: {error:?}");
        }
    };
    let qm_for_setup = queue_manager.clone_inner();

    append_log("startup.log", "queue manager initialized");
    crate::utils::log::log("queue manager initialized; managed_type=QueueManager");

    let result = tauri::Builder::default()
        .manage(queue_manager)
        .setup(move |app| {
            append_log("startup.log", "tauri setup begin");
            crate::utils::log::log("tauri setup begin; schedule manager starting");
            let schedule_manager = crate::services::schedule_manager::ScheduleManager::new(qm_for_setup.clone_inner());
            tauri::async_runtime::spawn(async move {
                append_log("startup.log", "schedule monitor started");
                schedule_manager.start_schedule_monitor().await;
                append_log("startup.log", "schedule monitor stopped");
            });

            // 创建系统托盘图标，用于窗口最小化到托盘后恢复
            let img = image::load_from_memory(include_bytes!("../icons/icon.png"))
                .expect("加载托盘图标失败")
                .to_rgba8();
            let (width, height) = img.dimensions();
            let icon = Image::new_owned(img.into_raw(), width, height);
            let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;
            TrayIconBuilder::with_id("main-tray")
                .icon(icon)
                .tooltip("alist-uploader")
                .menu(&menu)
                .on_menu_event(|app_handle, event| {
                    match event.id.as_ref() {
                        "show" => {
                            if let Some(window) = app_handle.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            app_handle.exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::Click {
                        button: tauri::tray::MouseButton::Left,
                        button_state: tauri::tray::MouseButtonState::Up,
                        ..
                    } = event
                    {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            append_log("startup.log", "tauri setup complete");
            crate::utils::log::log("tauri setup complete; QueueManager should be managed");
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if let Some(qm) = window.try_state::<crate::services::queue_manager::QueueManager>() {
                    let config = qm.config.blocking_read();
                    if config.upload.minimize_on_close {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
            }
        })
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
            crate::commands::get_is_uploading,
            crate::commands::stop_after_current,
            crate::commands::retry_upload,
            crate::commands::test_alist_connection,
            crate::commands::get_file_info,
            crate::commands::get_data_path,
            crate::commands::check_health,
            crate::commands::alist_login,
            crate::commands::write_client_log,
            crate::commands::test_notification,
            crate::commands::alist_list_dir,
            crate::commands::get_blocked_files,
            crate::commands::remove_blocked_file,
            crate::commands::clear_blocked_files,
        ])
        .run(tauri::generate_context!());

    if let Err(error) = result {
        append_log("startup.log", &format!("tauri runtime error: {error:?}"));
        panic!("error while running tauri application: {error:?}");
    }
}

