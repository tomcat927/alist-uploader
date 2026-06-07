import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { AppConfig, DirItem } from '../types';

interface FolderPickerProps {
  value: string;
  onChange: (path: string) => void;
  disabled?: boolean;
}

export function FolderPicker({ value, onChange, disabled }: FolderPickerProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [currentPath, setCurrentPath] = useState('/');
  const [folders, setFolders] = useState<DirItem[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState('');

  const loadFolders = async (path: string) => {
    setIsLoading(true);
    setError('');
    try {
      await invoke('write_client_log', { message: `打开 Alist 目录浏览: path=${path}` });
      const config = await invoke<AppConfig>('get_config');
      const alistConfig = config.alist;
      
      if (!alistConfig.base_url) {
        setError('请先配置 Alist 服务地址');
        await invoke('write_client_log', { message: 'Alist 目录浏览失败: base_url 为空' });
        return;
      }

      if (!alistConfig.token) {
        setError('请先登录获取 Token');
        await invoke('write_client_log', { message: `Alist 目录浏览失败: token 为空, username=${alistConfig.username}` });
        return;
      }

      const rawItems = await invoke<string>('alist_list_dir', {
        config,
        path
      });
      const items = JSON.parse(rawItems || '[]') as DirItem[];
      const directories = items.filter(item => item.is_dir);
      await invoke('write_client_log', { message: `Alist 目录浏览成功: path=${path}, item_count=${items.length}, dir_count=${directories.length}` });
      setFolders(directories);
      setCurrentPath(path);
    } catch (err: any) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
      await invoke('write_client_log', { message: `Alist 目录浏览异常: path=${path}, error=${message}` });
    } finally {
      setIsLoading(false);
    }
  };

  useEffect(() => {
    if (isOpen) {
      loadFolders(value || '/');
    }
  }, [isOpen]);

  const handleSelectFolder = (folderName: string) => {
    const newPath = currentPath === '/' ? `/${folderName}` : `${currentPath}/${folderName}`;
    onChange(newPath);
    setIsOpen(false);
  };

  const handleNavigateUp = () => {
    if (currentPath === '/') return;
    const parentPath = currentPath.split('/').slice(0, -1).join('/') || '/';
    loadFolders(parentPath);
  };

  const handleSelectCurrent = () => {
    onChange(currentPath);
    setIsOpen(false);
  };

  return (
    <div className="folder-picker">
      <div className="folder-picker-input">
        <input
          type="text"
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder="/dav/path"
          disabled={disabled}
        />
        <button 
          type="button"
          onClick={() => setIsOpen(!isOpen)}
          disabled={disabled}
          className="secondary"
        >
          {isOpen ? '关闭' : '浏览'}
        </button>
      </div>

      {isOpen && (
        <div className="folder-picker-dialog">
          <div className="folder-picker-header">
            <button 
              onClick={handleNavigateUp} 
              disabled={currentPath === '/'}
              className="small"
            >
              上级
            </button>
            <span className="current-path">{currentPath}</span>
            <button onClick={handleSelectCurrent} className="primary small">
              选择此目录
            </button>
          </div>

          <div className="folder-picker-content">
            {isLoading ? (
              <div className="loading-folders">加载中...</div>
            ) : error ? (
              <div className="error-folders">{error}</div>
            ) : folders.length === 0 ? (
              <div className="empty-folders">此目录为空</div>
            ) : (
              <div className="folder-list">
                {folders.map((folder) => (
                  <div
                    key={folder.name}
                    className="folder-item"
                    onClick={() => handleSelectFolder(folder.name)}
                  >
                    <span className="folder-icon">[DIR]</span>
                    <span className="folder-name">{folder.name}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
