use std::fs::{self, OpenOptions};
use std::io::Write;
use std::sync::Mutex;
use chrono::Local;

static LOG_MUTEX: Mutex<()> = Mutex::new(());

fn get_log_dir() -> Option<std::path::PathBuf> {
    let Some(mut log_dir) = dirs::data_local_dir() else {
        return None;
    };

    log_dir.push("alist-uploader");
    log_dir.push("logs");

    if fs::create_dir_all(&log_dir).is_err() {
        return None;
    }

    Some(log_dir)
}

fn get_log_file() -> Option<std::path::PathBuf> {
    let mut log_dir = get_log_dir()?;
    let now = Local::now();
    log_dir.push(format!("alist-{}.log", now.format("%Y-%m-%d")));
    Some(log_dir)
}

pub fn log(message: &str) {
    let _guard = LOG_MUTEX.lock().ok();

    let Some(log_path) = get_log_file() else {
        return;
    };

    let timestamp = Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        let _ = writeln!(file, "[{}] {}", timestamp, message);
    }
}

pub fn log_error(message: &str, error: &dyn std::error::Error) {
    log(&format!("ERROR: {} - {}", message, error));
}

pub fn log_debug(message: &str) {
    log(&format!("DEBUG: {}", message));
}

pub fn log_info(message: &str) {
    log(&format!("INFO: {}", message));
}

pub fn log_warn(message: &str) {
    log(&format!("WARN: {}", message));
}

pub fn log_request(method: &str, url: &str, status: u16, duration_ms: u64) {
    log(&format!("REQUEST: {} {} -> {} ({}ms)", method, url, status, duration_ms));
}