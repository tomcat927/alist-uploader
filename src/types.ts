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
  speed: number;
}

export interface BlockedFileRecord {
  file_path: string;
  file_name: string;
  file_size: number;
  reason: string;
  blocked_at: string;
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
  exe_path: string;
  kill_on_exit: boolean;
  run_in_background: boolean;
  use_system_proxy: boolean;
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
  notify_on_start: boolean;
  notify_on_stop: boolean;
}

export interface NotificationConfig {
  enabled: boolean;
  webhook_url: string;
  channels: string[];
}

export interface UploadConfig {
  concurrency: number;
  max_retries: number;
  max_tasks_per_run: number;
  check_update_on_startup: boolean;
  speed_limit: number;
  as_task: boolean;
  upload_method: string;
  last_alist_path: string;
  block_files_over_5gb: boolean;
  warn_files_over_4gb: boolean;
  file_exists_strategy: {
    strategy: string;
  };
  show_progress: boolean;
 notify_on_complete: boolean;
 notify_feishu_on_queue_complete: boolean;
 shutdown_after_complete: boolean;
 shutdown_delay_minutes: number;
 minimize_on_close: boolean;
  schedule?: ScheduledUpload;
  notification?: NotificationConfig;
}

export interface HistoryConfig {
  retention_days: number;
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
    exe_path: '',
    kill_on_exit: true,
    run_in_background: true,
    use_system_proxy: false,
  },
  upload: {
    concurrency: 1,
    max_retries: 5,
    max_tasks_per_run: 0,
    check_update_on_startup: true,
    speed_limit: 0,
    as_task: true,
    upload_method: 'stream',
    last_alist_path: '/',
    block_files_over_5gb: true,
    warn_files_over_4gb: true,
    file_exists_strategy: {
      strategy: 'ask',
    },
    show_progress: false,
   notify_on_complete: false,
   notify_feishu_on_queue_complete: false,
   shutdown_after_complete: false,
   shutdown_delay_minutes: 10,
   minimize_on_close: true,
    schedule: {
      enabled: false,
      start_time: '03:00',
      end_time: '07:00',
      notify_on_start: false,
      notify_on_stop: false,
    },
    notification: {
      enabled: false,
      webhook_url: '',
      channels: ['feishu'],
    },
  },
  history: {
    retention_days: 30,
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
