use reqwest::{Client, multipart, header::{HeaderMap, HeaderValue, AUTHORIZATION}};
use std::time::Duration;
use thiserror::Error;
use crate::utils::log::log;

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

#[derive(Debug, serde::Deserialize, Default)]
pub struct LoginResp {
    pub token: String,
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

    pub async fn login(&self, username: &str, password: &str) -> Result<String, AlistError> {
        let base_url = self.base_url.trim_end_matches('/');
        log(&format!("尝试登录 Alist: base_url={}, username={}, password_length={}", base_url, username, password.len()));

        let url = format!("{}/api/auth/login", base_url);

        let body = serde_json::json!({
            "username": username,
            "password": password
        });

        log(&format!("发送登录请求到: {}", url));

        let response = match self.client
            .post(&url)
            .json(&body)
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(err) => {
                log(&format!("登录请求失败: {}", err));
                return Err(AlistError::Request(err));
            }
        };

        let status = response.status();
        log(&format!("收到登录响应状态码: {}", status));

        let response_text = match response.text().await {
            Ok(text) => text,
            Err(err) => {
                log(&format!("读取登录响应正文失败: {}", err));
                return Err(AlistError::Api(format!("读取响应失败: {}", err)));
            }
        };
        log(&format!("登录响应正文已读取: status={}, body_length={}", status, response_text.len()));

        let resp: AlistResponse<LoginResp> = match serde_json::from_str(&response_text) {
            Ok(r) => r,
            Err(err) => {
                log(&format!("解析登录响应失败: status={}, error={}, body={}", status, err, response_text));
                return Err(AlistError::Api(format!("解析响应失败: {}; 原始响应: {}", err, response_text)));
            }
        };

        log(&format!("收到登录响应: http_status={}, code={}, message={}", status, resp.code, resp.message));

        if resp.code == 200 {
            match resp.data {
                Some(d) => {
                    log(&format!("登录成功，已获取 token，token_length={}", d.token.len()));
                    Ok(d.token)
                },
                None => {
                    log("登录成功但响应中未返回 token");
                    Err(AlistError::Api("登录成功但未返回 token".into()))
                }
            }
        } else {
            let msg = format!("登录失败: code={}, message={}", resp.code, resp.message);
            log(&msg);
            Err(AlistError::Api(resp.message))
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

    pub async fn check_service_available(&self) -> Result<bool, AlistError> {
        let url = format!("{}/ping", self.base_url.trim_end_matches('/'));

        let response = self.client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        match response {
            Ok(resp) => Ok(resp.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    pub async fn test_connection(&self) -> Result<bool, AlistError> {
        let url = format!("{}/api/me", self.base_url.trim_end_matches('/'));
        log(&format!("测试 Token 连接: url={}, has_token={}", url, !self.token.is_empty()));

        let response = self.client
            .get(&url)
            .headers(self.headers())
            .timeout(Duration::from_secs(10))
            .send()
            .await?;

        if response.status().is_success() {
            let resp: AlistResponse<AlistUserResp> = response.json().await?;
            Ok(resp.code == 200)
        } else {
            Ok(false)
        }
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

    pub async fn list_directory(&self, path: &str) -> Result<Vec<DirItem>, AlistError> {
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

        let resp: AlistResponse<ListResp> = response.json().await?;
        
        if resp.code == 200 {
            Ok(resp.data.map(|d| d.content).unwrap_or_default())
        } else {
            Err(AlistError::Api(resp.message))
        }
    }
}

#[derive(Debug, serde::Deserialize, Default)]
pub struct ListResp {
    pub content: Vec<DirItem>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DirItem {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sign: Option<String>,
}
