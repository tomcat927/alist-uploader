use std::path::Path;
use std::fs;
use thiserror::Error;
use walkdir::WalkDir;
use crate::models::FileInfo;

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

pub fn is_directory(path: &str) -> bool {
    Path::new(path).is_dir()
}

pub fn collect_files_from_dir(dir_path: &str) -> Result<Vec<FileInfo>, FsError> {
    let dir_path = Path::new(dir_path);
    
    if !dir_path.exists() || !dir_path.is_dir() {
        return Err(FsError::FileNotFound(dir_path.to_string_lossy().to_string()));
    }

    let mut files = Vec::new();
    
    for entry in WalkDir::new(dir_path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        let metadata = fs::metadata(path)?;
        
        if metadata.is_file() {
            let file_path = path.to_string_lossy().to_string();
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            let size = metadata.len();
            
            // 计算相对于根目录的相对路径，用于在 Alist 中保持目录结构
            let relative_path = path
                .strip_prefix(dir_path)
                .ok()
                .and_then(|p| p.to_str())
                .unwrap_or("");
            
            files.push(FileInfo {
                path: file_path,
                name: file_name,
                size,
                relative_path: Some(relative_path.to_string()),
            });
        }
    }

    Ok(files)
}

pub fn ensure_dir_exists(path: &Path) -> Result<(), std::io::Error> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    Ok(())
}
