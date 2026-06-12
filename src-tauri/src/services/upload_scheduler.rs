use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use crate::models::*;
use crate::services::alist_client::AlistClient;
use crate::services::queue_manager::QueueManager;
use crate::utils::log::log;

pub struct UploadScheduler {
    queue_manager: Arc<QueueManager>,
}

impl UploadScheduler {
    pub fn new(queue_manager: Arc<QueueManager>) -> Self {
        Self { queue_manager }
    }

    pub async fn start_scheduler(&self) {
        if self.queue_manager.is_uploading() {
            log("上传调度器已在运行中，跳过");
            return;
        }

        log("上传调度器启动");
        self.queue_manager.set_uploading(true);

        loop {
            if !self.queue_manager.is_uploading() {
                log("上传调度器收到停止信号，退出循环");
                break;
            }

            let config = self.queue_manager.config.read().await;
            
            if !self.can_start_new_task(&config).await {
                drop(config);
                sleep(Duration::from_millis(1000)).await;
                continue;
            }

            if let Some(task) = self.queue_manager.get_next_pending_task().await {
                log(&format!("取到待上传任务: file={}, size={}B, alist_path={}", task.file.name, task.file.size, task.alist_path));
                let task_clone = task.clone();
                let queue_manager = Arc::clone(&self.queue_manager);
                
                tokio::spawn(async move {
                    Self::execute_upload(queue_manager, task_clone).await;
                });

                if config.upload.concurrency == 1 {
                    drop(config);
                    while self.get_active_task_count().await > 0 {
                        sleep(Duration::from_millis(500)).await;
                    }
                }
            } else {
                drop(config);
                sleep(Duration::from_millis(1000)).await;
            }
        }

        self.queue_manager.set_uploading(false);
        log("上传调度器已停止");
    }

    async fn can_start_new_task(&self, config: &AppConfig) -> bool {
        // 如果已设置停止标志，不再启动新任务
        if self.queue_manager.stop_after_current() {
            return false;
        }
        
        let active_count = self.get_active_task_count().await;
        active_count < config.upload.concurrency as usize
    }

    async fn get_active_task_count(&self) -> usize {
        self.queue_manager.processing_tasks.len()
    }

    async fn execute_upload(queue_manager: Arc<QueueManager>, mut task: UploadTask) {
        log(&format!("开始上传: file={}, size={}B, alist_path={}, retry={}", task.file.name, task.file.size, task.alist_path, task.retry_count));
        
        let config = queue_manager.config.read().await;
        let alist_config = config.alist.clone();
        let upload_config = config.upload.clone();
        drop(config);
        
        task.mark_uploading();
        let _ = queue_manager.update_task(task.id.clone(), task.clone()).await;
        queue_manager.processing_tasks.insert(task.id.clone(), task.clone());

        let alist_client = AlistClient::new(
            alist_config.base_url.clone(),
            alist_config.token.clone(),
        );

        log(&format!("调用 Alist API 上传: file_path={}, alist_path={}, as_task={}", task.file.path, task.alist_path, upload_config.as_task));
        let result = Self::upload_with_retry(&queue_manager, &mut task, &alist_client, &upload_config).await;

        queue_manager.processing_tasks.remove(&task.id);

        match result {
            Ok(_) => {
                log(&format!("上传成功: file={}, size={}B, alist_path={}", task.file.name, task.file.size, task.alist_path));
                task.mark_completed();
                let _ = queue_manager.add_to_history(task.clone()).await;
                let _ = queue_manager.remove_completed_from_queue(task.id.clone()).await;
            }
            Err(e) => {
                log(&format!("上传出错: file={}, error={}, retry_count={}", task.file.name, e, task.retry_count));
                let config = queue_manager.config.read().await;
                let max_retries = config.upload.max_retries;
                if task.retry_count >= max_retries {
                    log(&format!("达到最大重试次数({})，标记为失败: file={}", max_retries, task.file.name));
                    task.mark_failed(e.clone());
                    let _ = queue_manager.add_to_history(task.clone()).await;
                    let _ = queue_manager.remove_completed_from_queue(task.id.clone()).await;
                    
                    let _ = queue_manager.mark_queue_failed(task.file.name.clone(), e.clone()).await;
                    
                    // 发送通知
                    let app_config = queue_manager.config.read().await;
                    if let Some(notification) = &app_config.upload.notification {
                        if notification.enabled && !notification.webhook_url.is_empty() {
                            Self::send_failure_notification(
                                &task.file.name,
                                &e,
                                notification,
                            ).await;
                        }
                    }
                    drop(app_config);
                    
                    // 停止整个队列
                    log(&format!("停止整个上传队列: 文件上传失败: file={}", task.file.name));
                    queue_manager.set_uploading(false);
                    queue_manager.set_stop_after_current(true);
                } else {
                    drop(config);
                    log(&format!("上传失败，准备重试: file={}, retry={}/{}", task.file.name, task.retry_count + 1, max_retries));
                    task.increment_retry();
                    let _ = queue_manager.update_task(task.id.clone(), task).await;
                }
            }
        }
    }

    async fn upload_with_retry(
        _queue_manager: &Arc<QueueManager>,
        task: &mut UploadTask,
        alist_client: &AlistClient,
        config: &UploadConfig,
    ) -> Result<(), String> {
        match alist_client.upload_file(&task.file.path, &task.alist_path, config.as_task, &config.upload_method).await {
            Ok(_) => Ok(()),
            Err(e) => {
                log(&format!("Alist API 上传失败: file={}, error={}", task.file.name, e));
                Err(e.to_string())
            }
        }
    }

    async fn send_failure_notification(
        file_name: &str,
        error: &str,
        notification: &NotificationConfig,
    ) {
        for channel in &notification.channels {
            match channel.as_str() {
                "feishu" => {
                    Self::send_feishu_notification(
                        &notification.webhook_url,
                        file_name,
                        error,
                    ).await;
                }
                _ => {
                    log::warn!("不支持的通知渠道: {}", channel);
                }
            }
        }
    }

    async fn send_feishu_notification(
        webhook_url: &str,
        file_name: &str,
        error: &str,
    ) {
        let message = format!(
            "## 上传失败通知\n\n\
            **文件**: {}\n\
            **错误**: {}\n\
            **状态**: 队列已停止，等待人工处理\n\n\
            _请检查文件路径、Alist 服务状态或网络连接_",
            file_name, error
        );

        let payload = serde_json::json!({
            "msg_type": "interactive",
            "card": {
                "header": {
                    "title": {
                        "tag": "plain_text",
                        "content": "上传失败通知"
                    },
                    "template": "red"
                },
                "elements": [{
                    "tag": "div",
                    "text": {
                        "tag": "lark_md",
                        "content": message
                    }
                }]
            }
        });

        if let Err(e) = reqwest::Client::new()
            .post(webhook_url)
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
        {
            log::error!("发送飞书通知失败: {}", e);
        } else {
            log::info!("飞书通知发送成功");
        }
    }

    pub fn stop_scheduler(&self) {
        self.queue_manager.set_uploading(false);
    }
}
