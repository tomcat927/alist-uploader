import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import type { UploadTask, AppConfig, TaskStatus } from '../types';

interface AppState {
  queue: UploadTask[];
  history: UploadTask[];
  config: AppConfig | null;
  isUploading: boolean;
  isLoading: boolean;
  alistConnected: boolean;
  alistChecking: boolean;
  
  // Actions
  loadQueue: () => Promise<void>;
  addToFileQueue: (filePath: string, alistPath: string) => Promise<UploadTask>;
  removeFromQueue: (taskId: string) => Promise<void>;
  clearQueue: () => Promise<void>;
  loadHistory: () => Promise<void>;
  clearHistory: () => Promise<void>;
  loadConfig: () => Promise<void>;
  saveConfig: (config: AppConfig) => Promise<void>;
  startUpload: () => Promise<void>;
  pauseUpload: () => Promise<void>;
  retryUpload: (taskId: string) => Promise<void>;
  testConnection: () => Promise<boolean>;
  checkHealth: () => Promise<void>;
  setIsUploading: (value: boolean) => void;
  startHealthCheck: () => void;
  stopHealthCheck: () => void;
}

let healthCheckInterval: ReturnType<typeof setInterval> | null = null;

export const useAppStore = create<AppState>((set, get) => ({
  queue: [],
  history: [],
  config: null,
  isUploading: false,
  isLoading: true,
  alistConnected: false,
  alistChecking: false,

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
    const task = await invoke<UploadTask>('add_to_queue', { filePath, alistPath });
    set(state => ({ queue: [...state.queue, task] }));
    return task;
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
    const config = await invoke<AppConfig>('get_config');
    set({ config });
  },

  saveConfig: async (config) => {
    await invoke('save_config', { config });
    set({ config });
  },

  startUpload: async () => {
    await invoke('start_upload');
    set({ isUploading: true });
  },

  pauseUpload: async () => {
    await invoke('pause_upload');
    set({ isUploading: false });
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

  testConnection: async () => {
    const config = get().config;
    if (!config) return false;
    return await invoke<boolean>('test_alist_connection', { config });
  },

  setIsUploading: (value) => {
    set({ isUploading: value });
  },

  checkHealth: async () => {
    const state = get();
    if (!state.config) return;
    
    try {
      const result = await invoke<boolean>('check_health', { config: state.config });
      set({ alistConnected: result, alistChecking: false });
    } catch (error) {
      set({ alistConnected: false, alistChecking: false });
    }
  },

  startHealthCheck: () => {
    if (healthCheckInterval) return;
    
    // 立即检查一次
    get().checkHealth();
    
    // 然后每 30 秒检查一次
    healthCheckInterval = setInterval(() => {
      get().checkHealth();
    }, 30000);
  },

  stopHealthCheck: () => {
    if (healthCheckInterval) {
      clearInterval(healthCheckInterval);
      healthCheckInterval = null;
    }
  },
}));
