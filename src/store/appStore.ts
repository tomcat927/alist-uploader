import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { DEFAULT_APP_CONFIG, normalizeAppConfig, type AddToQueueResult, type UploadTask, type AppConfig, type TaskStatus } from '../types';

interface AppState {
  queue: UploadTask[];
  history: UploadTask[];
  config: AppConfig;
  isUploading: boolean;
  isLoading: boolean;
  configLoaded: boolean;
  alistConnected: boolean;
  alistServiceAvailable: boolean;
  alistChecking: boolean;
  isStopping: boolean;

  // Actions
  loadQueue: () => Promise<void>;
  addToFileQueue: (filePath: string, alistPath: string) => Promise<AddToQueueResult>;
  removeFromQueue: (taskId: string) => Promise<void>;
  clearQueue: () => Promise<void>;
  loadHistory: () => Promise<void>;
  clearHistory: () => Promise<void>;
  loadConfig: () => Promise<void>;
  saveConfig: (config: AppConfig) => Promise<void>;
  startUpload: () => Promise<void>;
  pauseUpload: () => Promise<void>;
  retryUpload: (taskId: string) => Promise<void>;
  testConnection: (config?: AppConfig) => Promise<boolean>;
  checkHealth: () => Promise<void>;
  setIsUploading: (value: boolean) => void;
  setIsStopping: (value: boolean) => void;
  startHealthCheck: () => void;
  stopHealthCheck: () => void;
  fastCheckHealth: () => Promise<void>;
  login: (baseUrl: string, username: string, password: string) => Promise<void>;
}

let healthCheckInterval: ReturnType<typeof setInterval> | null = null;

export const useAppStore = create<AppState>((set, get) => ({
  queue: [],
  history: [],
  config: DEFAULT_APP_CONFIG,
  isUploading: false,
  isLoading: true,
  configLoaded: false,
  alistConnected: false,
  alistServiceAvailable: false,
  alistChecking: false,
  isStopping: false,

  loadQueue: async () => {
    try {
      const queue = await invoke<UploadTask[]>('get_queue');
      set({ queue, isLoading: false });
    } catch (error) {
      console.error('Failed to load queue:', error);
      set({ isLoading: false });
    }
  },

  addToFileQueue: async (filePath, alistPath) => {
    const result = await invoke<AddToQueueResult>('add_to_queue', { filePath, alistPath });
    set(state => ({ queue: [...state.queue, ...result.tasks] }));
    return result;
  },

  removeFromQueue: async (taskId) => {
    await invoke('remove_from_queue', { taskId });
    set(state => ({
      queue: state.queue.filter(t => t.id !== taskId)
    }));
  },

  clearQueue: async () => {
    await invoke('clear_queue');
    set({ queue: [] });
  },

  loadHistory: async () => {
    const history = await invoke<UploadTask[]>('get_history');
    set({ history });
  },

  clearHistory: async () => {
    await invoke('clear_history');
    set({ history: [] });
  },

  loadConfig: async () => {
    try {
      const config = await invoke<AppConfig>('get_config');
      set({ config: normalizeAppConfig(config), configLoaded: true });
    } catch (error) {
      console.error('Failed to load config:', error);
      set({ config: DEFAULT_APP_CONFIG, configLoaded: true });
    }
  },

  saveConfig: async (config) => {
    const normalizedConfig = normalizeAppConfig(config);
    await invoke('save_config', { config: normalizedConfig });
    set({ config: normalizedConfig, configLoaded: true });
  },

  startUpload: async () => {
    await invoke('start_upload');
    set({ isUploading: true });
  },

  pauseUpload: async () => {
    set({ isStopping: true });
    await invoke('stop_after_current');
    // 开始轮询检查是否已停止
    const checkStopped = async () => {
      const queue = await invoke<UploadTask[]>('get_queue');
      const uploadingTasks = queue.filter(t => t.status === 'uploading');
      if (uploadingTasks.length === 0) {
        set({ isUploading: false, isStopping: false });
      } else {
        setTimeout(checkStopped, 1000);
      }
    };
    checkStopped();
  },

  retryUpload: async (taskId) => {
    await invoke('retry_upload', { taskId });
    set(state => ({
      queue: state.queue.map(t => 
        t.id === taskId 
          ? { ...t, status: 'pending' as TaskStatus, retry_count: 0, error: undefined, progress: 0 }
          : t
      )
    }));
  },

  testConnection: async (config) => {
    const targetConfig = normalizeAppConfig(config ?? get().config);
    return await invoke<boolean>('test_alist_connection', { config: targetConfig });
  },

  setIsUploading: (value) => {
    set({ isUploading: value });
  },

  setIsStopping: (value) => {
    set({ isStopping: value });
  },

  checkHealth: async () => {
    const state = get();
    if (!state.config) return;

    try {
      const serviceAvailable = await invoke<boolean>('check_health', { config: state.config });

      if (serviceAvailable) {
        const loggedIn = await invoke<boolean>('test_alist_connection', { config: state.config });
        set({ alistServiceAvailable: true, alistConnected: loggedIn, alistChecking: false });
      } else {
        set({ alistServiceAvailable: false, alistConnected: false, alistChecking: false });
      }
    } catch (error) {
      console.error('Health check failed:', error);
      set({ alistServiceAvailable: false, alistConnected: false, alistChecking: false });
    }
  },

  login: async (baseUrl: string, username: string, password: string) => {
    console.log('[login] 开始登录流程:', { baseUrl, username });
    await invoke<string>('alist_login', { baseUrl, username, password });
    console.log('[login] 登录成功');
    await get().loadConfig();
    set({ alistConnected: true });
  },

  startHealthCheck: () => {
    if (healthCheckInterval) return;
    
    // 立即检查一次
    get().checkHealth();
    
    // 每 30 秒检查一次（常态）
    healthCheckInterval = setInterval(() => {
      get().checkHealth();
    }, 30000);
  },

  fastCheckHealth: async () => {
    // 加速检测：每 3 秒检查一次，直到连通或达到 20 次（约 60 秒）
    for (let i = 0; i < 20; i++) {
      await get().checkHealth();
      const state = get();
      if (state.alistConnected) break;
      await new Promise(resolve => setTimeout(resolve, 3000));
    }
  },

  stopHealthCheck: () => {
    if (healthCheckInterval) {
      clearInterval(healthCheckInterval);
      healthCheckInterval = null;
    }
  },
}));
