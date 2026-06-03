use std::sync::Arc;
use std::time::Duration;
use chrono::{Local, Timelike};
use tokio::time::sleep;
use crate::services::queue_manager::QueueManager;
use crate::services::upload_scheduler::UploadScheduler;

pub struct ScheduleManager {
    queue_manager: Arc<QueueManager>,
}

impl ScheduleManager {
    pub fn new(queue_manager: Arc<QueueManager>) -> Self {
        Self { queue_manager }
    }

    pub async fn start_schedule_monitor(&self) {
        loop {
            sleep(Duration::from_secs(30)).await; // 每 30 秒检查一次

            let config = self.queue_manager.config.read().await;
            let schedule = match &config.upload.schedule {
                Some(s) if s.enabled => s.clone(),
                _ => continue,
            };
            drop(config);

            let now = Local::now();
            let current_time = format!("{:02}:{:02}", now.hour(), now.minute());

            // 检查是否到达开始时间
            if current_time == schedule.start_time {
                // 确保不在上传中
                if !self.queue_manager.is_uploading() {
                    log::info!("定时上传时间到：{}", schedule.start_time);
                    
                    // 检查队列是否有任务
                    let queue = self.queue_manager.queue.read().await;
                    let has_pending = queue.tasks.iter().any(|t| t.status == crate::models::TaskStatus::Pending);
                    drop(queue);

                    if has_pending {
                        // 重置停止标志
                        self.queue_manager.set_stop_after_current(false);
                        
                        // 启动上传
                        let scheduler = UploadScheduler::new(self.queue_manager.clone_inner());
                        tokio::spawn(async move {
                            scheduler.start_scheduler().await;
                        });
                    }
                }
            }

            // 检查是否到达结束时间
            if current_time == schedule.end_time {
                log::info!("定时上传结束时间到：{}", schedule.end_time);
                // 设置停止标志（等待当前任务完成）
                self.queue_manager.set_stop_after_current(true);
            }
        }
    }
}
