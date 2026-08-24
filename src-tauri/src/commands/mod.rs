use tauri::State;
use crate::models::*;
use crate::services::queue_manager::QueueManager;
use crate::services::alist_client::AlistClient;
use crate::utils::storage::Storage;
use crate::utils::log::log;

#[tauri::command]
pub async fn get_queue(queue_manager: State<'_, QueueManager>) -> Result<Vec<UploadTask>, String> {
    let queue = queue_manager.queue.read().await;
    Ok(queue.tasks.clone())
}

#[tauri::command]
pub async fn add_to_queue(
    queue_manager: State<'_, QueueManager>,
    file_path: String,
    alist_path: String,
) -> Result<AddToQueueResult, String> {
    log(&format!("添加文件到队列: file_path={}, alist_path={}", file_path, alist_path));
    let result = queue_manager
        .add_to_queue(file_path, alist_path)
        .await
        .map_err(|e| e.to_string())?;
    log(&format!("添加到队列完成: 共 {} 个任务, warning_count={}", result.tasks.len(), result.warnings.len()));
    Ok(result)
}

#[tauri::command]
pub async fn remove_from_queue(
    queue_manager: State<'_, QueueManager>,
    task_id: String,
) -> Result<(), String> {
    queue_manager
        .remove_from_queue(task_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn clear_queue(queue_manager: State<'_, QueueManager>) -> Result<(), String> {
    queue_manager
        .clear_queue()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_history(queue_manager: State<'_, QueueManager>) -> Result<Vec<UploadTask>, String> {
    Ok(queue_manager.get_history().await)
}

#[tauri::command]
pub async fn clear_history(queue_manager: State<'_, QueueManager>) -> Result<(), String> {
    queue_manager
        .clear_history()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_config() -> Result<AppConfig, String> {
    let config = Storage::load_config().map_err(|e| {
        log(&format!("读取磁盘配置失败: {}", e));
        e.to_string()
    })?;
    log(&format!("读取磁盘配置: base_url={}, username={}, has_token={}, password_length={}, auto_login={}, last_alist_path={}", config.alist.base_url, config.alist.username, !config.alist.token.is_empty(), config.alist.password.len(), config.alist.auto_login, config.upload.last_alist_path));
    Ok(config)
}

#[tauri::command]
pub async fn save_config(queue_manager: State<'_, QueueManager>, config: AppConfig) -> Result<(), String> {
    log(&format!("收到保存配置请求: base_url={}, username={}, has_token={}, password_length={}, auto_login={}, last_alist_path={}", config.alist.base_url, config.alist.username, !config.alist.token.is_empty(), config.alist.password.len(), config.alist.auto_login, config.upload.last_alist_path));
    queue_manager.save_config(config).await.map_err(|e| {
        log(&format!("保存配置失败: {}", e));
        e.to_string()
    })?;
    log("配置保存成功");
    Ok(())
}

#[tauri::command]
pub async fn start_upload(queue_manager: State<'_, QueueManager>) -> Result<(), String> {
    log("收到开始上传请求");
    let scheduler = 
        crate::services::upload_scheduler::UploadScheduler::new(queue_manager.inner().clone_inner());
    
    tokio::spawn(async move {
        scheduler.start_scheduler().await;
    });

    Ok(())
}

#[tauri::command]
pub fn get_is_uploading(queue_manager: State<'_, QueueManager>) -> bool {
    queue_manager.is_uploading()
}

#[tauri::command]
pub async fn pause_upload(queue_manager: State<'_, QueueManager>) -> Result<(), String> {
    queue_manager.set_stop_after_current(true);
    Ok(())
}

#[tauri::command]
pub async fn stop_after_current(queue_manager: State<'_, QueueManager>) -> Result<(), String> {
    queue_manager.set_stop_after_current(true);
    Ok(())
}

#[tauri::command]
pub async fn retry_upload(
    queue_manager: State<'_, QueueManager>,
    task_id: String,
) -> Result<(), String> {
    let mut queue = queue_manager.queue.write().await;
    if let Some(task) = queue.tasks.iter_mut().find(|t| t.id == task_id) {
        task.status = TaskStatus::Pending;
        task.retry_count = 0;
        task.error = None;
        task.progress = 0;
        task.updated_at = chrono::Utc::now();
    }
    
    crate::utils::storage::Storage::save_queue(&*queue)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn test_alist_connection(config: AppConfig) -> Result<bool, String> {
    log(&format!("收到测试连接请求: base_url={}, username={}, has_token={}", config.alist.base_url, config.alist.username, !config.alist.token.is_empty()));

    let client = AlistClient::new(config.alist.base_url, config.alist.token);

    let result = client
        .test_connection()
        .await
        .map_err(|e| {
            log(&format!("测试连接异常: {}", e));
            e.to_string()
        })?;

    log(&format!("测试连接结果: {}", result));
    Ok(result)
}

#[tauri::command]
pub async fn get_file_info(path: String) -> Result<FileInfo, String> {
    let (size, name) = crate::utils::fs::get_file_info(&path)
        .await
        .map_err(|e| e.to_string())?;

    Ok(FileInfo { path, name, size, relative_path: None })
}

#[tauri::command]
pub async fn get_data_path() -> Result<String, String> {
    Ok(Storage::get_data_path()
        .to_string_lossy()
        .to_string())
}

#[tauri::command]
pub async fn check_health(config: AppConfig) -> Result<bool, String> {
    log(&format!("收到服务健康检查请求: base_url={}", config.alist.base_url));

    let client = AlistClient::new(config.alist.base_url, config.alist.token);

    let result = client
        .check_service_available()
        .await
        .map_err(|e| {
            log(&format!("服务健康检查异常: {}", e));
            e.to_string()
        })?;

    log(&format!("服务健康检查结果: {}", result));
    Ok(result)
}

#[tauri::command]
pub async fn alist_login(
    queue_manager: State<'_, QueueManager>,
    base_url: String,
    username: String,
    password: String,
) -> Result<String, String> {
    let normalized_base_url = base_url.trim_end_matches('/').to_string();
    log(&format!("开始 Alist 登录流程: base_url={}, username={}, password_length={}", normalized_base_url, username, password.len()));

    if normalized_base_url.is_empty() {
        log("Alist 登录失败: 服务地址为空");
        return Err("服务地址不能为空".to_string());
    }
    if username.trim().is_empty() {
        log("Alist 登录失败: 用户名为空");
        return Err("用户名不能为空".to_string());
    }
    if password.is_empty() {
        log("Alist 登录失败: 密码为空");
        return Err("密码不能为空".to_string());
    }

    let client = AlistClient::new(normalized_base_url.clone(), String::new());

    let token = client
        .login(&username, &password)
        .await
        .map_err(|e| {
            log(&format!("Alist 登录请求失败: base_url={}, username={}, error={}", normalized_base_url, username, e));
            e.to_string()
        })?;

    log(&format!("Alist 登录成功: username={}, token_length={}", username, token.len()));

    let mut config = Storage::load_config().map_err(|e| {
        log(&format!("读取配置失败，无法保存登录信息: {}", e));
        e.to_string()
    })?;

    config.alist.base_url = normalized_base_url.clone();
    config.alist.token = token.clone();
    config.alist.username = username.clone();
    config.alist.password = password;

    queue_manager.save_config(config.clone()).await.map_err(|e| {
        log(&format!("保存登录配置失败: base_url={}, username={}, has_token={}, error={}", normalized_base_url, username, !token.is_empty(), e));
        e.to_string()
    })?;

    log(&format!("登录配置已持久化: base_url={}, username={}, has_token={}, password_length={}", normalized_base_url, username, !token.is_empty(), config.alist.password.len()));
    Ok(token)
}

#[tauri::command]
pub async fn write_client_log(message: String) -> Result<(), String> {
    log(&format!("前端事件: {}", message));
    Ok(())
}

#[tauri::command]
pub async fn test_notification(config: NotificationConfig) -> Result<(), String> {
    log(&format!("收到测试通知请求: channels={:?}, has_webhook={}", config.channels, !config.webhook_url.is_empty()));

    if config.webhook_url.trim().is_empty() {
        return Err("Webhook URL 不能为空".to_string());
    }
    if config.channels.is_empty() {
        return Err("未配置通知渠道".to_string());
    }

    crate::services::upload_scheduler::UploadScheduler::test_notification(&config)
        .await
        .map_err(|e| {
            log(&format!("发送测试通知失败: {}", e));
            e
        })?;

    log("测试通知发送成功");
    Ok(())
}

#[tauri::command]
pub async fn get_blocked_files() -> Result<Vec<BlockedFileRecord>, String> {
    let data = crate::utils::storage::Storage::load_blocked_files().map_err(|e| e.to_string())?;
    Ok(data.records)
}

#[tauri::command]
pub async fn remove_blocked_file(index: usize) -> Result<(), String> {
    let mut data = crate::utils::storage::Storage::load_blocked_files().map_err(|e| e.to_string())?;
    if index < data.records.len() {
        data.records.remove(index);
        crate::utils::storage::Storage::save_blocked_files(&data).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn clear_blocked_files() -> Result<(), String> {
    crate::utils::storage::Storage::save_blocked_files(&crate::models::BlockedFileData::default()).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn alist_list_dir(config: AppConfig, path: String) -> Result<String, String> {
    log(&format!("收到 Alist 目录列表请求: base_url={}, username={}, has_token={}, path={}", config.alist.base_url, config.alist.username, !config.alist.token.is_empty(), path));
    let client = AlistClient::new(config.alist.base_url, config.alist.token);
    
    let items = client
        .list_directory(&path)
        .await
        .map_err(|e| {
            log(&format!("Alist 目录列表失败: path={}, error={}", path, e));
            e.to_string()
        })?;
    
    log(&format!("Alist 目录列表成功: path={}, item_count={}", path, items.len()));
    serde_json::to_string(&items).map_err(|e| {
        log(&format!("序列化 Alist 目录列表失败: path={}, error={}", path, e));
        e.to_string()
    })
}

#[tauri::command]
pub async fn get_shutdown_state(
    queue_manager: State<'_, QueueManager>,
) -> Result<Option<String>, String> {
    let deadline = queue_manager.get_shutdown_deadline().await;
    Ok(deadline.map(|d| d.to_rfc3339()))
}

#[tauri::command]
pub async fn cancel_shutdown(
    queue_manager: State<'_, QueueManager>,
) -> Result<(), String> {
    match std::process::Command::new("shutdown")
        .args(["/a"])
        .spawn()
    {
        Ok(_) => {
            queue_manager.clear_shutdown_deadline().await;
            log("已取消定时关机");
            Ok(())
        }
        Err(e) => {
            log(&format!("取消关机失败: {}", e));
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub async fn open_file_location(file_path: String) -> Result<(), String> {
    let path = std::path::Path::new(&file_path);
    if !path.exists() {
        return Err(format!("文件不存在: {}", file_path));
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer.exe")
            .args(["/select,", &file_path])
            .spawn()
            .map_err(|e| {
                log(&format!("打开文件所在目录失败: path={}, error={}", file_path, e));
                e.to_string()
            })?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(["-R", &file_path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        let parent = path.parent().unwrap_or(std::path::Path::new("."));
        std::process::Command::new("xdg-open")
            .arg(parent)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    log(&format!("已打开文件所在目录: {}", file_path));
    Ok(())
}
