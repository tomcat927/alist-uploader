use std::path::Path;
use std::fs;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FsError {
    #[error("文件不存在：{0}")]
    FileNotFound(String),
    #[error("无法访问文件：{0}")]
    AccessDenied(String),
    #[error("IO 错误：{0}")]
    Io(#[from] std::io::Error),
}

pub async fn get_file_info(path: &str) -> Result<(u64, String), FsError> {
    let path = Path::new(path);
    
    if !path.exists() {
        return Err(FsError::FileNotFound(path.to_string_lossy().to_string()));
    }

    let metadata = fs::metadata(path)?;
    let size = metadata.len();
    
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok((size, name))
}

pub fn ensure_dir_exists(path: &Path) -> Result<(), std::io::Error> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}
