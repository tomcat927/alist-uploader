import { listen } from '@tauri-apps/api/event';
import { useEffect } from 'react';
import { useAppStore } from '../store/appStore';

export function useFileDrop(alistPath: string) {
  const addToFileQueue = useAppStore((state) => state.addToFileQueue);

  useEffect(() => {
    // Tauri 原生拖拽事件
    const unlistenDragDrop = listen('tauri://file-drop', async (event) => {
      const paths = event.payload as string[];
      if (Array.isArray(paths)) {
        for (const filePath of paths) {
          try {
            await addToFileQueue(filePath, alistPath);
          } catch (error) {
            console.error('Failed to add file to queue:', filePath, error);
          }
        }
      }
    });

    // 拖拽悬停事件
    const unlistenDragOver = listen('tauri://drag-over', () => {
      // 可以在这里添加视觉反馈
    });

    return () => {
      unlistenDragDrop.then((fn) => fn());
      unlistenDragOver.then((fn) => fn());
    };
  }, [alistPath, addToFileQueue]);
}
