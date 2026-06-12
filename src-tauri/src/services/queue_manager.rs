pub use dashmap::DashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::models::*;
use crate::utils::storage::Storage;

pub struct QueueManager {
    pub queue: Arc<RwLock<QueueData>>,
    pub history: Arc<RwLock<HistoryData>>,
    pub config: Arc<RwLock<AppConfig>>,
    pub processing_tasks: Arc<DashMap<String, UploadTask>>,
    is_uploading: Arc<AtomicBool>,
    stop_after_current: Arc<AtomicBool>,
}

impl QueueManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let queue = Storage::load_queue().unwrap_or_default();
        let history = Storage::load_history().unwrap_or_default();
        let config = Storage::load_config().unwrap_or_default();

        Ok(Self {
            queue: Arc::new(RwLock::new(queue)),
            history: Arc::new(RwLock::new(history)),
            config: Arc::new(RwLock::new(config)),
            processing_tasks: Arc::new(DashMap::new()),
            is_uploading: Arc::new(AtomicBool::new(false)),
            stop_after_current: Arc::new(AtomicBool::new(false)),
        })
    }

    pub async fn add_to_queue(&self, file_path: String, alist_path: String) -> Result<Vec<UploadTask>, Box<dyn std::error::Error>> {
        let mut added_tasks = Vec::new();
        
        if crate::utils::fs::is_directory(&file_path) {
            // 如果是目录，递归收集所有文件
            let files = crate::utils::fs::collect_files_from_dir(&file_path)
                .map_err(|e| format!("收集文件夹文件失败: {}", e))?;
            
            for file_info in files {
                let task = self.add_single_file_to_queue(&file_info, &alist_path).await?;
                added_tasks.push(task);
            }
        } else {
            // 单个文件
            let (size, name) = crate::utils::fs::get_file_info(&file_path)
                .await
                .map_err(|e| e.to_string())?;
            
            let mut task = UploadTask::new(file_path.clone(), alist_path);
            task.file.size = size;
            task.file.name = name;
            
            let mut queue = self.queue.write().await;
            queue.tasks.push(task.clone());
            Storage::save_queue(&*queue)?;
            drop(queue);
            
            added_tasks.push(task);
        }
        
        Ok(added_tasks)
    }
    
    async fn add_single_file_to_queue(&self, file_info: &crate::models::FileInfo, alist_path: &str) -> Result<UploadTask, Box<dyn std::error::Error>> {
        // 如果有相对路径，则构建完整的目标路径
        let target_path = if let Some(ref relative_path) = file_info.relative_path {
            if relative_path.is_empty() {
                alist_path.to_string()
            } else {
                format!("{}/{}", alist_path.trim_end_matches('/'), relative_path.trim_start_matches('/'))
            }
        } else {
            alist_path.to_string()
        };
        
        let mut task = UploadTask::new(file_info.path.clone(), target_path);
        task.file.size = file_info.size;
        task.file.name = file_info.name.clone();
        task.file.relative_path = file_info.relative_path.clone();

        let mut queue = self.queue.write().await;
        queue.tasks.push(task.clone());
        Storage::save_queue(&*queue)?;
        drop(queue);

        Ok(task)
    }

    pub async fn remove_from_queue(&self, task_id: String) -> Result<(), Box<dyn std::error::Error>> {
        let mut queue = self.queue.write().await;
        queue.tasks.retain(|t| t.id != task_id);
        Storage::save_queue(&*queue)?;
        Ok(())
    }

    pub async fn clear_queue(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut queue = self.queue.write().await;
        queue.tasks.clear();
        Storage::save_queue(&*queue)?;
        Ok(())
    }

    pub async fn get_history(&self) -> Vec<UploadTask> {
        let history = self.history.read().await;
        history.records.clone()
    }

    pub async fn clear_history(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut history = self.history.write().await;
        history.records.clear();
        Storage::save_history(&*history)?;
        Ok(())
    }

    pub async fn save_config(&self, config: AppConfig) -> Result<(), Box<dyn std::error::Error>> {
        let mut config_guard = self.config.write().await;
        *config_guard = config.clone();
        Storage::save_config(&config)?;
        Ok(())
    }

    pub async fn update_task(&self, task_id: String, task: UploadTask) -> Result<(), Box<dyn std::error::Error>> {
        let mut queue = self.queue.write().await;
        if let Some(existing) = queue.tasks.iter_mut().find(|t| t.id == task_id) {
            *existing = task;
        }
        Storage::save_queue(&*queue)?;
        Ok(())
    }

    pub async fn get_next_pending_task(&self) -> Option<UploadTask> {
        let queue = self.queue.read().await;
        queue.tasks.iter()
            .find(|t| t.status == TaskStatus::Pending)
            .cloned()
    }

    pub async fn remove_completed_from_queue(&self, task_id: String) -> Result<(), Box<dyn std::error::Error>> {
        let mut queue = self.queue.write().await;
        queue.tasks.retain(|t| t.id != task_id);
        Storage::save_queue(&*queue)?;
        Ok(())
    }

    pub async fn add_to_history(&self, task: UploadTask) -> Result<(), Box<dyn std::error::Error>> {
        let mut history = self.history.write().await;
        history.records.insert(0, task);
        
        let config = self.config.read().await;
        let max_records = config.history.max_records;
        if history.records.len() > max_records {
            history.records.truncate(max_records);
        }
        
        Storage::save_history(&*history)?;
        Ok(())
    }

    pub fn is_uploading(&self) -> bool {
        self.is_uploading.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn set_uploading(&self, value: bool) {
        self.is_uploading.store(value, std::sync::atomic::Ordering::SeqCst);
    }

    pub fn clone_inner(&self) -> Arc<QueueManager> {
        Arc::new(Self {
            queue: Arc::clone(&self.queue),
            history: Arc::clone(&self.history),
            config: Arc::clone(&self.config),
            processing_tasks: Arc::clone(&self.processing_tasks),
            is_uploading: Arc::clone(&self.is_uploading),
            stop_after_current: Arc::clone(&self.stop_after_current),
        })
    }

    pub fn stop_after_current(&self) -> bool {
        self.stop_after_current.load(Ordering::SeqCst)
    }

    pub fn set_stop_after_current(&self, value: bool) {
        self.stop_after_current.store(value, Ordering::SeqCst);
    }

    pub async fn mark_queue_failed(
        &self,
        file_name: String,
        error: String,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 记录失败信息到日志
        log::error!(
            "队列因文件 '{}' 失败而停止: {}",
            file_name,
            error
        );
        
        Ok(())
    }
}
