use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use crate::models::*;
use crate::services::alist_client::AlistClient;
use crate::services::queue_manager::{is_root_alist_path, QueueManager, FOUR_GB, FIVE_GB};
use crate::services::rate_limiter::RateLimiter;
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

        // 重置停止标志，确保新上传可以正常启动
       self.queue_manager.set_stop_after_current(false);
       self.queue_manager.reset_tasks_uploaded();

       // 取消上一轮可能遗留的定时关机
       if self.queue_manager.get_shutdown_deadline().await.is_some() {
           let _ = std::process::Command::new("shutdown").args(["/a"]).spawn();
           self.queue_manager.clear_shutdown_deadline().await;
           log("启动新上传任务，已取消上一轮遗留的定时关机");
       }

       let scheduler_start = chrono::Local::now();

       log("上传调度器启动");
        self.queue_manager.set_uploading(true);
        let config = self.queue_manager.config.read().await;
        let rate_limiter = Arc::new(RateLimiter::new(config.upload.speed_limit));
        drop(config);

        let mut max_tasks_reached_logged = false;

        loop {
            if !self.queue_manager.is_uploading() {
                log("上传调度器收到停止信号，退出循环");
                break;
            }

            let config = self.queue_manager.config.read().await;

            if config.upload.max_tasks_per_run > 0
                && self.queue_manager.tasks_uploaded_in_run() >= config.upload.max_tasks_per_run
            {
                let limit = config.upload.max_tasks_per_run;
                drop(config);
                if self.get_active_task_count().await == 0 {
                    log(&format!("已达到本次上传任务数上限（{} 个），上传调度器自动结束", limit));
                    break;
                }
                if !max_tasks_reached_logged {
                    log(&format!("已达到本次上传任务数上限（{} 个），等待当前任务完成后结束", limit));
                    max_tasks_reached_logged = true;
                }
                sleep(Duration::from_millis(1000)).await;
                continue;
            }
            
            if !self.can_start_new_task(&config).await {
                drop(config);
                // 尽管停止标志已设置，也检查是否所有任务都已完成
                if self.get_active_task_count().await == 0 {
                    log("停止标志已设置且无活动任务，上传调度器结束");
                    break;
                }
                sleep(Duration::from_millis(1000)).await;
                continue;
            }

            if let Some(task) = self.queue_manager.claim_next_pending_task().await {
                log(&format!("取到待上传任务: file={}, size={}B, alist_path={}", task.file.name, task.file.size, task.alist_path));
                let task_clone = task.clone();
                let queue_manager = Arc::clone(&self.queue_manager);
                self.queue_manager.processing_tasks.insert(task.id.clone(), task.clone());
                
                let rate_limiter_clone = Arc::clone(&rate_limiter);
                tokio::spawn(async move {
                    Self::execute_upload(queue_manager, task_clone, rate_limiter_clone).await;
                });

                if config.upload.concurrency == 1 {
                    drop(config);
                    while self.get_active_task_count().await > 0 {
                        sleep(Duration::from_millis(500)).await;
                    }
                }
            } else if self.get_active_task_count().await == 0 {
                log("没有待上传任务且无活动任务，上传调度器自动结束");
                break;
            } else {
                drop(config);
                sleep(Duration::from_millis(1000)).await;
            }
       }

       // 队列自然完成或达到任务上限时发送飞书通知（手动停止或失败停止不发）
       if !self.queue_manager.stop_after_current() {
           let config = self.queue_manager.config.read().await;
           if config.upload.notify_feishu_on_queue_complete
               && (self.queue_manager.tasks_uploaded_in_run() > 0
                   || self.queue_manager.tasks_failed_in_run() > 0)
           {
               if let Some(notification) = &config.upload.notification {
                   if notification.enabled && !notification.webhook_url.is_empty() {
                       let succeeded = self.queue_manager.tasks_uploaded_in_run();
                       let failed = self.queue_manager.tasks_failed_in_run();
                       let total = succeeded + failed;
                       let end_time = chrono::Local::now();
                       Self::send_queue_complete_notification(
                           notification,
                           total,
                           succeeded,
                           failed,
                           scheduler_start,
                           end_time,
                       )
                       .await;
                   }
              }
          }

           if config.upload.shutdown_after_complete
               && (self.queue_manager.tasks_uploaded_in_run() > 0
                   || self.queue_manager.tasks_failed_in_run() > 0)
           {
               let delay_minutes = config.upload.shutdown_delay_minutes;
               let delay_seconds = delay_minutes * 60;
               match std::process::Command::new("shutdown")
                   .args(["/s", "/t", &delay_seconds.to_string()])
                   .spawn()
               {
                   Ok(_) => {
                       let deadline = chrono::Utc::now()
                           + chrono::Duration::seconds(delay_seconds as i64);
                       self.queue_manager.set_shutdown_deadline(deadline).await;
                       log(&format!(
                           "已调度关机: delay={}分钟({}秒后)",
                           delay_minutes, delay_seconds
                       ));
                   }
                   Err(e) => {
                       log(&format!("调度关机失败: {}", e));
                   }
               }
           }
          drop(config);
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

    async fn execute_upload(
        queue_manager: Arc<QueueManager>,
        mut task: UploadTask,
        rate_limiter: Arc<RateLimiter>,
    ) {
        log(&format!("开始上传: file={}, size={}B, alist_path={}, retry={}", task.file.name, task.file.size, task.alist_path, task.retry_count));
        if is_root_alist_path(&task.alist_path) {
            let error = "上传目标目录不能为根目录 /，请选择 Alist 中的具体目录".to_string();
            log(&format!("上传任务被拦截: task_id={}, file={}, alist_path=/, error={}", task.id, task.file.name, error));
            task.mark_failed(error);
            let _ = queue_manager.add_to_history(task.clone()).await;
            let _ = queue_manager.remove_completed_from_queue(task.id.clone()).await;
            queue_manager.processing_tasks.remove(&task.id);
            return;
        }
        
        let config = queue_manager.config.read().await;
        let alist_config = config.alist.clone();
        let upload_config = config.upload.clone();
        drop(config);

        if upload_config.block_files_over_5gb && task.file.size > FIVE_GB {
            let error = format!("115 网盘非会员单个文件最大支持 5GB，{} 超过限制，已阻止上传。", task.file.name);
            log(&format!("上传任务被大文件保护拦截: task_id={}, file={}, size={}B, error={}", task.id, task.file.name, task.file.size, error));
            task.mark_failed(error);
            let _ = queue_manager.add_to_history(task.clone()).await;
            let _ = queue_manager.remove_completed_from_queue(task.id.clone()).await;
            queue_manager.processing_tasks.remove(&task.id);
            return;
        }

        if upload_config.warn_files_over_4gb && task.file.size > FOUR_GB {
            log(&format!("上传大文件风险提示: task_id={}, file={}, size={}B, message=超过4GB，可能因1小时内未完成导致Token过期", task.id, task.file.name, task.file.size));
        }
        
        let alist_client = AlistClient::new(
            alist_config.base_url.clone(),
            alist_config.token.clone(),
        );

        log(&format!("调用 Alist API 上传: file_path={}, alist_path={}, as_task={}", task.file.path, task.alist_path, upload_config.as_task));
        let result = Self::upload_with_retry(
            &queue_manager,
            &mut task,
            &alist_client,
            &upload_config,
            rate_limiter,
        ).await;

        queue_manager.processing_tasks.remove(&task.id);

        match result {
            Ok(_) => {
                log(&format!("上传成功: file={}, size={}B, alist_path={}", task.file.name, task.file.size, task.alist_path));
                task.mark_completed();
                queue_manager.increment_tasks_uploaded();
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
                   queue_manager.increment_tasks_failed();
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
        queue_manager: &Arc<QueueManager>,
        task: &mut UploadTask,
        alist_client: &AlistClient,
        config: &UploadConfig,
        rate_limiter: Arc<RateLimiter>,
    ) -> Result<(), String> {
        match alist_client.upload_file(
            &task.file.path,
            &task.alist_path,
            config.as_task,
            &config.upload_method,
            Some(rate_limiter),
        ).await {
            Ok(Some(alist_task_id)) => {
                log(&format!("等待 Alist 后台上传任务完成: file={}, alist_task_id={}", task.file.name, alist_task_id));
                Self::wait_for_alist_task(queue_manager, task, alist_client, &alist_task_id).await
            }
            Ok(None) => Ok(()),
            Err(e) => {
                log(&format!("Alist API 上传失败: file={}, error={}", task.file.name, e));
                Err(e.to_string())
            }
        }
    }

    async fn wait_for_alist_task(
        queue_manager: &Arc<QueueManager>,
        task: &mut UploadTask,
        alist_client: &AlistClient,
        alist_task_id: &str,
    ) -> Result<(), String> {
        let mut missing_checks = 0;

        loop {
            sleep(Duration::from_secs(2)).await;

            let tasks = alist_client.get_upload_tasks().await.map_err(|e| {
                log(&format!("查询 Alist 后台上传任务失败: file={}, alist_task_id={}, error={}", task.file.name, alist_task_id, e));
                e.to_string()
            })?;

            let Some(alist_task) = tasks.into_iter().find(|item| item.id == alist_task_id) else {
                let exists = alist_client.check_file_exists(&task.alist_path, &task.file.name).await.map_err(|e| {
                    log(&format!("确认 Alist 后台上传结果失败: file={}, alist_task_id={}, error={}", task.file.name, alist_task_id, e));
                    e.to_string()
                })?;

                if exists {
                    log(&format!("Alist 后台上传任务已从未完成列表消失且目标文件存在: file={}, alist_task_id={}", task.file.name, alist_task_id));
                    task.progress = 100;
                    task.speed = 0;
                    let _ = queue_manager.update_task(task.id.clone(), task.clone()).await;
                    return Ok(());
                }

                missing_checks += 1;
                if missing_checks < 3 {
                    log(&format!("Alist 后台上传任务已从未完成列表消失，等待目标文件出现在目录中: file={}, alist_task_id={}, check={}/3", task.file.name, alist_task_id, missing_checks));
                    continue;
                }

                let error = format!("Alist 后台上传任务已消失，但目标目录中未找到文件 {}", task.file.name);
                log(&format!("Alist 后台上传结果确认失败: file={}, alist_task_id={}, error={}", task.file.name, alist_task_id, error));
                return Err(error);
            };

            missing_checks = 0;

            let progress_f = alist_task.progress.clamp(0.0, 100.0);
            let progress = progress_f as u8;
            let now = chrono::Utc::now();

            // 速度计算：仅当进度推进时，用 (Δprogress × size / 100) / Δt 估算字节/秒
            // 进度未变则保留上次速度（与 OpenList 前端一致）；首次记录基线，不产出速度
            if task.prev_ts.is_none() {
                task.prev_progress = progress_f;
                task.prev_ts = Some(now);
            } else if (progress_f - task.prev_progress).abs() > f64::EPSILON {
                let prev_ts = task.prev_ts.unwrap();
                let dt = (now - prev_ts).num_milliseconds();
                if dt > 0 {
                    let delta_bytes = ((progress_f - task.prev_progress).abs() / 100.0) * task.file.size as f64;
                    let speed = (delta_bytes / dt as f64 * 1000.0) as u64;
                    task.speed = speed;
                }
                task.prev_progress = progress_f;
                task.prev_ts = Some(now);
            }

            if task.progress != progress || task.speed > 0 {
                task.progress = progress;
                let _ = queue_manager.update_task(task.id.clone(), task.clone()).await;
            }

            log(&format!(
                "Alist 后台上传任务状态: file={}, alist_task_id={}, state={}, status={}, progress={}%, error={}",
                task.file.name,
                alist_task_id,
                alist_task.state,
                alist_task.status,
                alist_task.progress,
                alist_task.error
            ));

            if alist_task.state == 2 {
                log(&format!("Alist 后台上传任务完成: file={}, alist_task_id={}", task.file.name, alist_task_id));
                task.progress = 100;
                task.speed = 0;
                let _ = queue_manager.update_task(task.id.clone(), task.clone()).await;
                return Ok(());
            }

            if alist_task.state == 3 || alist_task.state == 4 || !alist_task.error.is_empty() {
                let error = if alist_task.error.is_empty() {
                    format!("Alist 后台上传任务失败: state={}, status={}", alist_task.state, alist_task.status)
                } else {
                    alist_task.error
                };
                log(&format!("Alist 后台上传任务失败: file={}, alist_task_id={}, error={}", task.file.name, alist_task_id, error));
                return Err(error);
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

        if let Err(e) = Self::post_feishu_card(webhook_url, &payload).await {
            log::error!("发送飞书通知失败: {}", e);
        } else {
            log::info!("飞书通知发送成功");
        }
    }

    pub async fn test_notification(notification: &NotificationConfig) -> Result<(), String> {
        let message = "## 测试通知\n\n\
            **内容**: 这是一条来自 Alist Uploader 的测试通知\n\
            **状态**: 通知配置有效\n\n\
            _如果你看到这条消息，说明 Webhook 配置正确_";

        let payload = serde_json::json!({
            "msg_type": "interactive",
            "card": {
                "header": {
                    "title": {
                        "tag": "plain_text",
                        "content": "Alist Uploader 测试通知"
                    },
                    "template": "green"
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

        for channel in &notification.channels {
            match channel.as_str() {
                "feishu" => {
                    Self::post_feishu_card(&notification.webhook_url, &payload)
                        .await
                        .map_err(|e| format!("发送飞书测试通知失败: {}", e))?;
                }
                _ => {
                    log::warn!("不支持的通知渠道: {}", channel);
                }
            }
        }

        log::info!("测试通知发送成功");
        Ok(())
    }
   async fn send_queue_complete_notification(
       notification: &NotificationConfig,
       total: u32,
       succeeded: u32,
       failed: u32,
       start_time: chrono::DateTime<chrono::Local>,
       end_time: chrono::DateTime<chrono::Local>,
   ) {
       let duration = end_time - start_time;
       let duration_str = format_duration(duration);
       let start_str = start_time.format("%Y-%m-%d %H:%M:%S").to_string();
       let end_str = end_time.format("%Y-%m-%d %H:%M:%S").to_string();

       let (title, template) = if failed > 0 {
           ("上传队列完成（含失败）", "orange")
       } else {
           ("上传队列完成", "green")
       };

       let message = format!(
           "## 上传队列已完成\n\n\
            **开始时间**: {}\n\
            **结束时间**: {}\n\
            **耗时**: {}\n\
            **总任务数**: {}\n\
            **成功**: {}\n\
            **失败**: {}",
           start_str, end_str, duration_str, total, succeeded, failed
       );

       let payload = serde_json::json!({
           "msg_type": "interactive",
           "card": {
               "header": {
                   "title": {
                       "tag": "plain_text",
                       "content": title
                   },
                   "template": template
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

       for channel in &notification.channels {
           match channel.as_str() {
               "feishu" => {
                   if let Err(e) = Self::post_feishu_card(&notification.webhook_url, &payload).await {
                       log::error!("发送队列完成通知失败: {}", e);
                   } else {
                       log::info!("队列完成通知发送成功: total={}, succeeded={}, failed={}", total, succeeded, failed);
                   }
               }
               _ => {
                   log::warn!("不支持的通知渠道: {}", channel);
               }
           }
       }
   }

   pub async fn send_schedule_notification(
        notification: &NotificationConfig,
        event_type: &str,
    ) {
        let (title, message, template) = match event_type {
            "start" => (
                "定时上传开始",
                "## 定时上传开始\n\n**状态**: 已到达定时上传的开始时间，调度器已启动\n\n_系统将自动扫描队列中的待上传文件并开始上传_",
                "blue",
            ),
            "stop" => (
                "定时上传结束",
                "## 定时上传结束\n\n**状态**: 已到达定时上传的结束时间，不再启动新上传任务\n\n_如果当前正在上传，当前任务会继续完成_",
                "yellow",
            ),
            _ => return,
        };
        let payload = serde_json::json!({
            "msg_type": "interactive",
            "card": {
                "header": {
                    "title": {
                        "tag": "plain_text",
                        "content": title,
                    },
                    "template": template,
                },
                "elements": [{
                    "tag": "div",
                    "text": {
                        "tag": "lark_md",
                        "content": message,
                    }
                }]
            }
        });
        for channel in &notification.channels {
            match channel.as_str() {
                "feishu" => {
                    if let Err(e) = Self::post_feishu_card(&notification.webhook_url, &payload).await {
                        log::error!("发送定时上传通知失败: {}, event_type={}", e, event_type);
                    } else {
                        log::info!("定时上传通知发送成功: event_type={}", event_type);
                    }
                }
                _ => {
                    log::warn!("不支持的通知渠道: {}", channel);
                }
            }
        }
    }

    async fn post_feishu_card(webhook_url: &str, payload: &serde_json::Value) -> Result<(), reqwest::Error> {
        reqwest::Client::new()
            .post(webhook_url)
            .header("Content-Type", "application/json")
            .json(payload)
            .send()
            .await
            .map(|_| ())
    }

   pub fn stop_scheduler(&self) {
       self.queue_manager.set_uploading(false);
   }
}

fn format_duration(duration: chrono::Duration) -> String {
    let total_secs = duration.num_seconds();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}小时{}分钟{}秒", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}分钟{}秒", minutes, seconds)
    } else {
        format!("{}秒", seconds)
    }
}

