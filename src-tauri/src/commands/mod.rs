use tauri::State;
use crate::models::*;
use crate::services::queue_manager::QueueManager;
use crate::services::alist_client::AlistClient;
use crate::utils::storage::Storage;
use tokio::sync::RwLock;

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
) -> Result<UploadTask, String> {
    queue_manager
        .add_to_queue(file_path, alist_path)
        .await
        .map_err(|e| e.to_string())
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
pub async fn get_config(queue_manager: State<'_, QueueManager>) -> Result<AppConfig, String> {
    let config = queue_manager.config.read().await;
    Ok(config.clone())
}

#[tauri::command]
pub async fn save_config(
    queue_manager: State<'_, QueueManager>,
    config: AppConfig,
) -> Result<(), String> {
    queue_manager
        .save_config(config)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn start_upload(queue_manager: State<'_, QueueManager>) -> Result<(), String> {
    let scheduler = 
        crate::services::upload_scheduler::UploadScheduler::new(queue_manager.inner().clone_inner());
    
    tokio::spawn(async move {
        scheduler.start_scheduler().await;
    });

    Ok(())
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
    let client = AlistClient::new(config.alist.base_url, config.alist.token);
    
    client
        .test_connection()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_file_info(path: String) -> Result<FileInfo, String> {
    let (size, name) = crate::utils::fs::get_file_info(&path)
        .await
        .map_err(|e| e.to_string())?;

    Ok(FileInfo { path, name, size })
}

#[tauri::command]
pub async fn get_data_path() -> Result<String, String> {
    Ok(Storage::get_data_path()
        .to_string_lossy()
        .to_string())
}

#[tauri::command]
pub async fn check_health(config: AppConfig) -> Result<bool, String> {
    let client = AlistClient::new(config.alist.base_url, config.alist.token);

    client
        .check_service_available()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn alist_login(
    queue_manager: State<'_, QueueManager>,
    base_url: String,
    username: String,
    password: String,
) -> Result<String, String> {
    let client = AlistClient::new(base_url.clone(), String::new());
    
    let token = client
        .login(&username, &password)
        .await
        .map_err(|e| e.to_string())?;
    
    // 获取当前配置并更新
    let mut config = queue_manager.config.write().await;
    config.alist.base_url = base_url;
    config.alist.token = token.clone();
    config.alist.username = username;
    config.alist.password = password;
    
    // 保存配置
    drop(config);
    queue_manager.save_config(queue_manager.config.read().await.clone()).await
        .map_err(|e| e.to_string())?;
    
    Ok(token)
}

#[tauri::command]
pub async fn alist_list_dir(config: AppConfig, path: String) -> Result<String, String> {
    let client = AlistClient::new(config.alist.base_url, config.alist.token);
    
    let items = client
        .list_directory(&path)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(serde_json::to_string(&items).unwrap_or_default())
}
