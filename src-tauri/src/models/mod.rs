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
    }

    pub fn mark_completed(&mut self) {
        self.status = TaskStatus::Completed;
        self.end_time = Some(Utc::now());
        self.progress = 100;
        self.updated_at = Utc::now();
        if let Some(start) = self.start_time {
            self.duration = Some((Utc::now() - start).num_seconds() as u64);
        }
    }

    pub fn mark_failed(&mut self, error: String) {
        self.status = TaskStatus::Failed;
        self.end_time = Some(Utc::now());
        self.error = Some(error);
        self.updated_at = Utc::now();
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
}

impl Default for AlistConfig {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:5244".to_string(),
            token: String::new(),
            username: String::new(),
            password: String::new(),
        }
    }
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
pub struct UploadConfig {
    pub concurrency: u8,
    pub max_retries: u32,
    pub as_task: bool,
    pub file_exists_strategy: FileExistsStrategy,
    pub show_progress: bool,
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            concurrency: 1,
            max_retries: 5,
            as_task: true,
            file_exists_strategy: FileExistsStrategy::default(),
            show_progress: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryConfig {
    pub max_records: usize,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self { max_records: 100 }
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
