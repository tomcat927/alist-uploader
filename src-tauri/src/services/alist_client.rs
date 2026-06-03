use reqwest::{Client, multipart, header::{HeaderMap, HeaderValue, AUTHORIZATION}};
use std::time::Duration;
use thiserror::Error;
use crate::models::*;

#[derive(Error, Debug)]
pub enum AlistError {
    #[error("HTTP 请求失败：{0}")]
    Request(#[from] reqwest::Error),
    #[error("Alist API 错误：{0}")]
    Api(String),
    #[error("文件已存在")]
    FileExists,
    #[error("认证失败")]
    AuthFailed,
    #[error("服务不可用")]
    ServiceUnavailable,
    #[error("IO 错误：{0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, serde::Deserialize)]
pub struct AlistResponse<T> {
    pub code: i32,
    pub message: String,
    #[serde(default)]
    pub data: Option<T>,
}

#[derive(Debug, serde::Deserialize, Default)]
pub struct AlistUserResp {
    pub id: i64,
    pub username: String,
    pub role: bool,
}

#[derive(Debug, serde::Deserialize)]
pub struct AlistTaskResp {
    pub id: String,
    pub name: String,
    pub state: i32, // 0: pending, 1: running, 2: succeeded, 3: cancelled, 4: error
    pub progress: i32,
}

#[derive(Debug, serde::Deserialize, Default)]
pub struct AlistTaskListResp {
    pub tasks: Vec<AlistTaskResp>,
    pub total: i32,
}

pub struct AlistClient {
    client: Client,
    base_url: String,
    token: String,
}

impl AlistClient {
    pub fn new(base_url: String, token: String) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(3600))
            .build()
            .unwrap();

        Self {
            client,
            base_url,
            token,
        }
    }

    fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        
        if !self.token.is_empty() {
            let auth_value = HeaderValue::from_str(&self.token);
            if let Ok(val) = auth_value {
                headers.insert(AUTHORIZATION, val);
            }
        }
        
        headers
    }

    pub async fn test_connection(&self) -> Result<bool, AlistError> {
        let url = format!("{}/api/me", self.base_url);
        
        let response = self.client
            .get(&url)
            .headers(self.headers())
            .send()
            .await?;

        Ok(response.status().is_success())
    }

    pub async fn get_current_user(&self) -> Result<AlistUserResp, AlistError> {
        let url = format!("{}/api/me", self.base_url);
        
        let response = self.client
            .get(&url)
            .headers(self.headers())
            .send()
            .await?;

        let resp: AlistResponse<AlistUserResp> = response.json().await?;
        
        if resp.code == 200 {
            resp.data.ok_or_else(|| AlistError::Api("无用户数据".into()))
        } else {
            Err(AlistError::Api(resp.message))
        }
    }

    pub async fn check_file_exists(&self, path: &str, filename: &str) -> Result<bool, AlistError> {
        let url = format!("{}/api/fs/list", self.base_url);
        
        let body = serde_json::json!({
            "path": path
        });

        let response = self.client
            .post(&url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;

        let resp: AlistResponse<serde_json::Value> = response.json().await?;
        
        if resp.code == 200 {
            if let Some(data) = resp.data {
                if let Some(content) = data.get("content") {
                    if let Some(files) = content.as_array() {
                        return Ok(files.iter().any(|f| {
                            f.get("name")
                                .and_then(|n| n.as_str())
                                .map(|n| n == filename)
                                .unwrap_or(false)
                        }));
                    }
                }
            }
            Ok(false)
        } else {
            // 404 可能意味着目录不存在，文件肯定不存在
            Ok(false)
        }
    }

    pub async fn upload_file(
        &self,
        file_path: &str,
        alist_path: &str,
        as_task: bool,
    ) -> Result<Option<String>, AlistError> {
        let url = format!("{}/api/fs/put", self.base_url);
        
        let file_data = tokio::fs::read(file_path).await?;
        
        let form = multipart::Form::new()
            .part("file", multipart::Part::bytes(file_data).file_name(
                file_path.split('/').last().or_else(|| file_path.split('\\').last()).unwrap_or("unknown").to_string()
            ));

        let mut request = self.client
            .put(&url)
            .headers(self.headers())
            .query(&[("path", &format!("{}/{}", alist_path, 
                file_path.split('/').last().or_else(|| file_path.split('\\').last()).unwrap_or("unknown")))])
            .multipart(form);

        if as_task {
            request = request.query(&[("as_task", "true")]);
        }

        let response = request.send().await?;
        
        let resp: AlistResponse<serde_json::Value> = response.json().await?;
        
        if resp.code == 200 {
            Ok(None)
        } else {
            Err(AlistError::Api(resp.message))
        }
    }

    pub async fn get_upload_tasks(&self) -> Result<Vec<AlistTaskResp>, AlistError> {
        let url = format!("{}/api/admin/task/upload/list", self.base_url);
        
        let response = self.client
            .get(&url)
            .headers(self.headers())
            .query(&[("page", "1"), ("per_page", "100")])
            .send()
            .await?;

        let resp: AlistResponse<AlistTaskListResp> = response.json().await?;
        
        if resp.code == 200 {
            Ok(resp.data.map(|d| d.tasks).unwrap_or_default())
        } else {
            Err(AlistError::Api(resp.message))
        }
    }

    pub async fn get_task_progress(&self, task_id: &str) -> Result<AlistTaskResp, AlistError> {
        let tasks = self.get_upload_tasks().await?;
        
        tasks.into_iter()
            .find(|t| t.id == task_id)
            .ok_or_else(|| AlistError::Api(format!("任务 {} 未找到", task_id)))
    }
}
