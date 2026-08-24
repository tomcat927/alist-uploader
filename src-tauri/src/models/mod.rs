use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Uploading,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadTask {
    pub id: String,
    pub file: FileInfo,
    pub alist_path: String,
    pub status: TaskStatus,
    pub progress: u8,
    pub retry_count: u32,
    pub error: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub duration: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// 上传速度（字节/秒），仅 status=uploading 时有效，前端用于显示
    #[serde(default)]
    pub speed: u64,
    /// 上一次轮询的进度百分比（0.0-100.0），仅内存使用，不持久化
    #[serde(skip)]
    pub prev_progress: f64,
    /// 上一次轮询的时间戳，仅内存使用，不持久化
    #[serde(skip)]
    pub prev_ts: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddToQueueResult {
    pub tasks: Vec<UploadTask>,
    pub warnings: Vec<String>,
}

impl UploadTask {
    pub fn new(file_path: String, alist_path: String) -> Self {
        let file_name = file_path
            .split('/')
            .last()
            .or_else(|| file_path.split('\\').last())
            .unwrap_or("unknown")
            .to_string();

        Self {
            id: Uuid::new_v4().to_string(),
            file: FileInfo {
                path: file_path,
                name: file_name,
                size: 0,
                relative_path: None,
            },
            alist_path,
            status: TaskStatus::Pending,
            progress: 0,
            retry_count: 0,
            error: None,
            start_time: None,
            end_time: None,
            duration: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            speed: 0,
            prev_progress: 0.0,
            prev_ts: None,
        }
    }

    pub fn update_status(&mut self, status: TaskStatus) {
        self.status = status;
        self.updated_at = Utc::now();
    }

    pub fn mark_uploading(&mut self) {
        self.status = TaskStatus::Uploading;
        self.start_time = Some(Utc::now());
        self.updated_at = Utc::now();
        self.error = None;
        self.speed = 0;
        self.prev_progress = 0.0;
        self.prev_ts = None;
    }

    pub fn mark_completed(&mut self) {
        self.status = TaskStatus::Completed;
        self.end_time = Some(Utc::now());
        self.progress = 100;
        self.updated_at = Utc::now();
        self.speed = 0;
        if let Some(start) = self.start_time {
            self.duration = Some((Utc::now() - start).num_seconds() as u64);
        }
    }

    pub fn mark_failed(&mut self, error: String) {
        self.status = TaskStatus::Failed;
        self.end_time = Some(Utc::now());
        self.error = Some(error);
        self.updated_at = Utc::now();
        self.speed = 0;
        if let Some(start) = self.start_time {
            self.duration = Some((Utc::now() - start).num_seconds() as u64);
        }
    }

    pub fn increment_retry(&mut self) {
        self.retry_count += 1;
        self.status = TaskStatus::Pending;
        self.updated_at = Utc::now();
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlistConfig {
    pub base_url: String,
    pub token: String,
    pub username: String,
    pub password: String,
    #[serde(default = "default_auto_login")]
    pub auto_login: bool,
}

impl Default for AlistConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:5244".to_string(),
            token: String::new(),
            username: String::new(),
            password: String::new(),
            auto_login: true,
        }
    }
}

fn default_auto_login() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileExistsStrategy {
    #[serde(rename = "strategy")]
    pub value: String, // "ask", "overwrite", "skip", "rename"
}

impl Default for FileExistsStrategy {
    fn default() -> Self {
        Self {
            value: "ask".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduledUpload {
    pub enabled: bool,
    pub start_time: String, // "HH:MM" format
    pub end_time: String,   // "HH:MM" format
    #[serde(default)]
    pub notify_on_start: bool,
    #[serde(default)]
    pub notify_on_stop: bool,
}

impl Default for ScheduledUpload {
    fn default() -> Self {
        Self {
            enabled: false,
            start_time: "03:00".to_string(),
            end_time: "07:00".to_string(),
            notify_on_start: false,
            notify_on_stop: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadConfig {
    pub concurrency: u8,
    pub max_retries: u32,
    /// 上传限速（字节/秒），0 表示不限速
    #[serde(default)]
    pub speed_limit: u64,
    pub as_task: bool,
    #[serde(default = "default_upload_method")]
    pub upload_method: String,
    #[serde(default = "default_alist_path")]
    pub last_alist_path: String,
    #[serde(default = "default_true")]
    pub block_files_over_5gb: bool,
    #[serde(default = "default_true")]
    pub warn_files_over_4gb: bool,
    pub file_exists_strategy: FileExistsStrategy,
    pub show_progress: bool,
   #[serde(default)]
   pub notify_on_complete: bool,
   #[serde(default)]
   pub notify_feishu_on_queue_complete: bool,
   #[serde(default)]
   pub shutdown_after_complete: bool,
   #[serde(default = "default_shutdown_delay_minutes")]
   pub shutdown_delay_minutes: u32,
   #[serde(default)]
   pub minimize_on_close: bool,
    /// 每轮上传任务数上限，0 表示不限
    #[serde(default)]
    pub max_tasks_per_run: u32,
    pub schedule: Option<ScheduledUpload>,
    pub notification: Option<NotificationConfig>,
}

fn default_upload_method() -> String {
    "stream".to_string()
}

fn default_shutdown_delay_minutes() -> u32 {
    10
}

fn default_alist_path() -> String {
    "/".to_string()
}

fn default_true() -> bool {
    true
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            concurrency: 1,
            max_retries: 5,
            speed_limit: 0,
            as_task: true,
            upload_method: "stream".to_string(),
            last_alist_path: "/".to_string(),
            block_files_over_5gb: true,
            warn_files_over_4gb: true,
            file_exists_strategy: FileExistsStrategy::default(),
            show_progress: false,
           notify_on_complete: false,
           notify_feishu_on_queue_complete: false,
           shutdown_after_complete: false,
           shutdown_delay_minutes: 10,
           minimize_on_close: true,
            max_tasks_per_run: 0,
            schedule: Some(ScheduledUpload::default()),
            notification: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub enabled: bool,
    pub webhook_url: String,
    pub channels: Vec<String>, // "feishu", "dingtalk", etc.
}

impl Default for NotificationConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            webhook_url: String::new(),
            channels: vec!["feishu".to_string()],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedFileRecord {
    pub file_path: String,
    pub file_name: String,
    pub file_size: u64,
    pub reason: String,
    pub blocked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedFileData {
    pub records: Vec<BlockedFileRecord>,
}

impl Default for BlockedFileData {
    fn default() -> Self {
        Self { records: vec![] }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    #[serde(default = "default_history_retention_days")]
    pub retention_days: u32,
}

fn default_history_retention_days() -> u32 {
    30
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self { retention_days: 30 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub alist: AlistConfig,
    pub upload: UploadConfig,
    pub history: HistoryConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            alist: AlistConfig::default(),
            upload: UploadConfig::default(),
            history: HistoryConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueData {
    pub tasks: Vec<UploadTask>,
    pub version: u32,
}

impl Default for QueueData {
    fn default() -> Self {
        Self {
            tasks: Vec::new(),
            version: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryData {
    pub records: Vec<UploadTask>,
    pub version: u32,
}

impl Default for HistoryData {
    fn default() -> Self {
        Self {
            records: Vec::new(),
            version: 1,
        }
    }
}
