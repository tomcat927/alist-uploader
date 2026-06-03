import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { DirItem } from '../types';

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
      const config = JSON.parse(localStorage.getItem('tauri:config') || '{}');
      const alistConfig = config?.upload?.alist || { base_url: '', token: '' };
      
      if (!alistConfig.base_url) {
        setError('请先配置 Alist 服务地址');
        return;
      }

      const items = await invoke<DirItem[]>('alist_list_dir', {
        config: alistConfig,
        path
      });
      
      setFolders(items.filter(item => item.is_dir));
      setCurrentPath(path);
    } catch (err: any) {
      setError(err.toString());
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
              ⬆ 上级
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
                    <span className="folder-icon">📁</span>
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
