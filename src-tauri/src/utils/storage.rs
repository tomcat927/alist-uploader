use std::path::PathBuf;
use std::fs;
use std::io::{Read, Write};
use serde::Serialize;
use serde::de::DeserializeOwned;
use dirs::config_dir;
use crate::models::*;

const APP_NAME: &str = "alist-uploader";

pub struct Storage;

impl Storage {
    fn get_data_dir() -> PathBuf {
        let config_dir = config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(APP_NAME);
        
        if !config_dir.exists() {
            fs::create_dir_all(&config_dir).ok();
        }
        
        config_dir
    }

    fn read_json<T: DeserializeOwned + Default>(filename: &str) -> Result<T, Box<dyn std::error::Error>> {
        let path = Self::get_data_dir().join(filename);
        
        if !path.exists() {
            return Ok(T::default());
        }

        let mut file = fs::File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        
        Ok(serde_json::from_str(&contents)?)
    }

    fn write_json<T: Serialize>(filename: &str, data: &T) -> Result<(), Box<dyn std::error::Error>> {
        let path = Self::get_data_dir().join(filename);
        let json = serde_json::to_string_pretty(data)?;
        
        let mut file = fs::File::create(path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        
        Ok(())
    }

    pub fn load_queue() -> Result<QueueData, Box<dyn std::error::Error>> {
        Self::read_json("queue.json")
    }

    pub fn save_queue(queue: &QueueData) -> Result<(), Box<dyn std::error::Error>> {
        Self::write_json("queue.json", queue)
    }

    pub fn load_history() -> Result<HistoryData, Box<dyn std::error::Error>> {
        Self::read_json("history.json")
    }

    pub fn save_history(history: &HistoryData) -> Result<(), Box<dyn std::error::Error>> {
        Self::write_json("history.json", history)
    }

    pub fn load_config() -> Result<AppConfig, Box<dyn std::error::Error>> {
        Self::read_json("config.json")
    }

    pub fn save_config(config: &AppConfig) -> Result<(), Box<dyn std::error::Error>> {
        Self::write_json("config.json", config)
    }

    pub fn load_blocked_files() -> Result<BlockedFileData, Box<dyn std::error::Error>> {
        Self::read_json("blocked_files.json")
    }

    pub fn save_blocked_files(data: &BlockedFileData) -> Result<(), Box<dyn std::error::Error>> {
        Self::write_json("blocked_files.json", data)
    }

    pub fn get_data_path() -> PathBuf {
        Self::get_data_dir()
    }
}
