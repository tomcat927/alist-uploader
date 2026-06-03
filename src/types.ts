export type TaskStatus = 'pending' | 'uploading' | 'completed' | 'failed' | 'cancelled';

export interface FileInfo {
  path: string;
  name: string;
  size: number;
}

export interface UploadTask {
  id: string;
  file: FileInfo;
  alist_path: string;
  status: TaskStatus;
  progress: number;
  retry_count: number;
  error?: string;
  start_time?: string;
  end_time?: string;
  duration?: number;
  created_at: string;
  updated_at: string;
}

export interface AlistConfig {
  base_url: string;
  token: string;
  username: string;
  password: string;
}

export interface UploadConfig {
  concurrency: number;
  max_retries: number;
  as_task: boolean;
  file_exists_strategy: {
    strategy: string;
  };
  show_progress: boolean;
}

export interface HistoryConfig {
  max_records: number;
}

export interface AppConfig {
  alist: AlistConfig;
  upload: UploadConfig;
  history: HistoryConfig;
}
