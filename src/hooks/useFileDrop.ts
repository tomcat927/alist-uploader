import { getCurrentWebview } from '@tauri-apps/api/webview';
import { useEffect } from 'react';
import { useAppStore } from '../store/appStore';

export function useFileDrop(alistPath: string) {
  const addToFileQueue = useAppStore((state) => state.addToFileQueue);

  useEffect(() => {
    let unlistenFn: (() => void) | null = null;

    const setupDragDrop = async () => {
      try {
        const unlisten = await getCurrentWebview().onDragDropEvent(async (event) => {
          if (event.payload.type === 'drop') {
            const paths = event.payload.paths;
            if (Array.isArray(paths)) {
              for (const filePath of paths) {
                try {
                  await addToFileQueue(filePath, alistPath);
                } catch (error) {
                  console.error('Failed to add file to queue:', filePath, error);
                }
              }
            }
          } else if (event.payload.type === 'over') {
            // 可以在这里添加视觉反馈
          }
        });

        unlistenFn = unlisten;
      } catch (error) {
        console.error('Failed to setup drag drop:', error);
      }
    };

    setupDragDrop();

    return () => {
      if (unlistenFn) {
        unlistenFn();
      }
    };
  }, [alistPath, addToFileQueue]);
}
