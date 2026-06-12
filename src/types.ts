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

export interface AddToQueueResult {
  tasks: UploadTask[];
  warnings: string[];
}

export interface AlistConfig {
  base_url: string;
  token: string;
  username: string;
  password: string;
  auto_login: boolean;
}

export interface DirItem {
  name: string;
  size: number;
  is_dir: boolean;
  modified?: string;
  sign?: string;
}

export interface ScheduledUpload {
  enabled: boolean;
  start_time: string;
  end_time: string;
}

export interface NotificationConfig {
  enabled: boolean;
  webhook_url: string;
  channels: string[];
}

export interface UploadConfig {
  concurrency: number;
  max_retries: number;
  as_task: boolean;
  upload_method: string;
  last_alist_path: string;
  block_files_over_5gb: boolean;
  warn_files_over_4gb: boolean;
  file_exists_strategy: {
    strategy: string;
  };
  show_progress: boolean;
  schedule?: ScheduledUpload;
  notification?: NotificationConfig;
}

export interface HistoryConfig {
  max_records: number;
}

export interface AppConfig {
  alist: AlistConfig;
  upload: UploadConfig;
  history: HistoryConfig;
}

export const DEFAULT_APP_CONFIG: AppConfig = {
  alist: {
    base_url: 'http://127.0.0.1:5244',
    token: '',
    username: '',
    password: '',
    auto_login: true,
  },
  upload: {
    concurrency: 1,
    max_retries: 5,
    as_task: true,
    upload_method: 'stream',
    last_alist_path: '/',
    block_files_over_5gb: true,
    warn_files_over_4gb: true,
    file_exists_strategy: {
      strategy: 'ask',
    },
    show_progress: false,
    schedule: {
      enabled: false,
      start_time: '03:00',
      end_time: '07:00',
    },
    notification: {
      enabled: false,
      webhook_url: '',
      channels: ['feishu'],
    },
  },
  history: {
    max_records: 100,
  },
};

export const normalizeAppConfig = (config?: Partial<AppConfig> | null): AppConfig => ({
  alist: {
    ...DEFAULT_APP_CONFIG.alist,
    ...config?.alist,
  },
  upload: {
    ...DEFAULT_APP_CONFIG.upload,
    ...config?.upload,
    file_exists_strategy: {
      ...DEFAULT_APP_CONFIG.upload.file_exists_strategy,
      ...config?.upload?.file_exists_strategy,
    },
    schedule: {
      ...DEFAULT_APP_CONFIG.upload.schedule!,
      ...config?.upload?.schedule,
    },
    notification: {
      ...DEFAULT_APP_CONFIG.upload.notification!,
      ...config?.upload?.notification,
    },
  },
  history: {
    ...DEFAULT_APP_CONFIG.history,
    ...config?.history,
  },
});
