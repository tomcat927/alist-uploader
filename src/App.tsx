import { useEffect, useState } from 'react';
import { useAppStore } from './store/appStore';
import { open } from '@tauri-apps/plugin-dialog';
import { listen } from '@tauri-apps/api/event';
import type { AppConfig } from './types';
import './App.css';

function App() {
  const {
    queue,
    history,
    config,
    isUploading,
    isLoading,
    alistConnected,
    alistChecking,
    loadQueue,
    addToFileQueue,
    removeFromQueue,
    clearQueue,
    loadHistory,
    clearHistory,
    loadConfig,
    saveConfig,
    startUpload,
    pauseUpload,
    retryUpload,
    testConnection,
    setIsUploading,
    startHealthCheck,
    stopHealthCheck,
  } = useAppStore();

  const [alistPath, setAlistPath] = useState('/');
  const [activeTab, setActiveTab] = useState<'queue' | 'history' | 'settings'>('queue');
  const [configForm, setConfigForm] = useState<AppConfig | null>(null);
  const [connectionStatus, setConnectionStatus] = useState<'idle' | 'success' | 'error'>('idle');

  useEffect(() => {
    loadQueue();
    loadHistory();
    loadConfig();
    
    // 启动心跳检测
    startHealthCheck();
    
    // 监听文件拖拽事件
    const unlisten = listen('tauri://file-drop', async (event) => {
      const paths = event.payload as string[];
      if (Array.isArray(paths)) {
        for (const filePath of paths) {
          try {
            await addToFileQueue(filePath, alistPath);
          } catch (error) {
            console.error('Failed to add file:', error);
          }
        }
      }
    });

    return () => {
      stopHealthCheck();
      unlisten.then((fn) => fn());
    };
  }, [alistPath]);

  useEffect(() => {
    if (config) {
      setConfigForm({ ...config });
    }
  }, [config]);

  const handleSelectFiles = async () => {
    try {
      const selected = await open({
        multiple: true,
      });
      
      if (selected) {
        const files = Array.isArray(selected) ? selected : [selected];
        for (const file of files) {
          await addToFileQueue(file, alistPath);
        }
      }
    } catch (error) {
      console.error('Failed to select files:', error);
    }
  };

  const handleStartUpload = async () => {
    await startUpload();
  };

  const handlePauseUpload = async () => {
    await pauseUpload();
    setIsUploading(false);
  };

  const handleTestConnection = async () => {
    if (!configForm) return;
    
    try {
      const success = await testConnection();
      setConnectionStatus(success ? 'success' : 'error');
      setTimeout(() => setConnectionStatus('idle'), 3000);
    } catch (error) {
      setConnectionStatus('error');
      setTimeout(() => setConnectionStatus('idle'), 3000);
    }
  };

  const handleSaveConfig = async () => {
    if (!configForm) return;
    await saveConfig(configForm);
  };

  const formatFileSize = (bytes: number) => {
    const units = ['B', 'KB', 'MB', 'GB'];
    let size = bytes;
    let unitIndex = 0;
    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex++;
    }
    return `${size.toFixed(2)} ${units[unitIndex]}`;
  };

  const formatDateTime = (isoString?: string) => {
    if (!isoString) return '-';
    return new Date(isoString).toLocaleString('zh-CN');
  };

  if (isLoading) {
    return <div className="loading">加载中...</div>;
  }

  return (
    <div className="app">
      <header className="app-header">
        <h1>Alist 上传管理器</h1>
        <div className="header-status">
          <div className="alist-status">
            <span className={`status-dot ${alistChecking ? 'checking' : alistConnected ? 'connected' : 'disconnected'}`} />
            <span className="status-label">
              {alistChecking ? '检测中...' : alistConnected ? 'Alist 已连接' : 'Alist 未连接'}
            </span>
          </div>
          <span className={`status-indicator ${isUploading ? 'active' : ''}`}>
            {isUploading ? '上传中' : '空闲'}
          </span>
        </div>
      </header>

      <nav className="app-nav">
        <button 
          className={activeTab === 'queue' ? 'active' : ''}
          onClick={() => setActiveTab('queue')}
        >
          待上传队列 ({queue.length})
        </button>
        <button 
          className={activeTab === 'history' ? 'active' : ''}
          onClick={() => setActiveTab('history')}
        >
          历史记录 ({history.length})
        </button>
        <button 
          className={activeTab === 'settings' ? 'active' : ''}
          onClick={() => setActiveTab('settings')}
        >
          设置
        </button>
      </nav>

      <main className="app-content">
        {activeTab === 'queue' && (
          <div className="queue-tab">
            <div className="queue-toolbar">
              <div className="upload-path-input">
                <label>Alist 上传路径:</label>
                <input
                  type="text"
                  value={alistPath}
                  onChange={(e) => setAlistPath(e.target.value)}
                  placeholder="/dav/path"
                />
              </div>
              <div className="toolbar-actions">
                <button onClick={handleSelectFiles} disabled={isUploading}>
                  选择文件
                </button>
                {!isUploading ? (
                  <button 
                    onClick={handleStartUpload} 
                    disabled={queue.length === 0}
                    className="primary"
                  >
                    开始上传
                  </button>
                ) : (
                  <button onClick={handlePauseUpload} className="warning">
                    暂停
                  </button>
                )}
                <button onClick={clearQueue} disabled={queue.length === 0}>
                  清空队列
                </button>
              </div>
            </div>

            <div className="queue-list">
              {queue.length === 0 ? (
                <div className="empty-state">
                  <p>队列为空</p>
                  <p>拖拽文件到此处，或点击"选择文件"按钮添加</p>
                </div>
              ) : (
                <table>
                  <thead>
                    <tr>
                      <th>文件名</th>
                      <th>大小</th>
                      <th>目标路径</th>
                      <th>状态</th>
                      <th>重试次数</th>
                      <th>操作</th>
                    </tr>
                  </thead>
                  <tbody>
                    {queue.map(task => (
                      <tr key={task.id}>
                        <td>{task.file.name}</td>
                        <td>{formatFileSize(task.file.size)}</td>
                        <td>{task.alist_path}</td>
                        <td>
                          <span className={`status-badge status-${task.status}`}>
                            {task.status === 'pending' && '等待中'}
                            {task.status === 'uploading' && '上传中'}
                            {task.status === 'completed' && '已完成'}
                            {task.status === 'failed' && '失败'}
                            {task.status === 'cancelled' && '已取消'}
                          </span>
                        </td>
                        <td>{task.retry_count}</td>
                        <td>
                          {task.status === 'failed' && (
                            <button 
                              onClick={() => retryUpload(task.id)}
                              className="small"
                            >
                              重试
                            </button>
                          )}
                          <button 
                            onClick={() => removeFromQueue(task.id)}
                            className="small danger"
                          >
                            删除
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          </div>
        )}

        {activeTab === 'history' && (
          <div className="history-tab">
            <div className="history-toolbar">
              <button onClick={clearHistory} disabled={history.length === 0}>
                清空历史
              </button>
            </div>

            <div className="history-list">
              {history.length === 0 ? (
                <div className="empty-state">
                  <p>暂无历史记录</p>
                </div>
              ) : (
                <table>
                  <thead>
                    <tr>
                      <th>文件名</th>
                      <th>大小</th>
                      <th>目标路径</th>
                      <th>状态</th>
                      <th>完成时间</th>
                      <th>耗时</th>
                    </tr>
                  </thead>
                  <tbody>
                    {history.map(task => (
                      <tr key={task.id}>
                        <td>{task.file.name}</td>
                        <td>{formatFileSize(task.file.size)}</td>
                        <td>{task.alist_path}</td>
                        <td>
                          <span className={`status-badge status-${task.status} ${task.error ? 'with-error' : ''}`}>
                            {task.status === 'completed' ? '成功' : '失败'}
                            {task.error && `: ${task.error}`}
                          </span>
                        </td>
                        <td>{formatDateTime(task.end_time)}</td>
                        <td>{task.duration ? `${task.duration}s` : '-'}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          </div>
        )}

        {activeTab === 'settings' && configForm && (
          <div className="settings-tab">
            <div className="settings-section">
              <h3>Alist 账号配置</h3>
              <div className="form-group">
                <label>服务地址:</label>
                <input
                  type="text"
                  value={configForm.alist.base_url}
                  onChange={(e) => setConfigForm({
                    ...configForm,
                    alist: { ...configForm.alist, base_url: e.target.value }
                  })}
                  placeholder="http://127.0.0.1:5244"
                />
              </div>
              <div className="form-group">
                <label>用户名:</label>
                <input
                  type="text"
                  value={configForm.alist.username}
                  onChange={(e) => setConfigForm({
                    ...configForm,
                    alist: { ...configForm.alist, username: e.target.value }
                  })}
                  placeholder="填写你的 Alist 账号"
                />
              </div>
              <div className="form-group">
                <label>密码:</label>
                <input
                  type="password"
                  value={configForm.alist.password}
                  onChange={(e) => setConfigForm({
                    ...configForm,
                    alist: { ...configForm.alist, password: e.target.value }
                  })}
                  placeholder="填写你的 Alist 密码"
                />
              </div>
              <div className="toolbar-actions">
                <button 
                  onClick={async () => {
                    try {
                      useAppStore.getState().login(
                        configForm.alist.base_url,
                        configForm.alist.username,
                        configForm.alist.password
                      );
                      setConnectionStatus('success');
                      setTimeout(() => setConnectionStatus('idle'), 3000);
                    } catch (error) {
                      setConnectionStatus('error');
                      setTimeout(() => setConnectionStatus('idle'), 3000);
                    }
                  }} 
                  className="secondary"
                  disabled={!configForm.alist.username || !configForm.alist.password}
                >
                  登录获取 Token
                </button>
                <button onClick={handleTestConnection} className="secondary">
                  测试连接
                </button>
              </div>
              {connectionStatus === 'success' && (
                <span className="test-result success">
                  {configForm.alist.token ? 'Token 已缓存' : '连接成功'}
                </span>
              )}
              {connectionStatus === 'error' && (
                <span className="test-result error">认证失败，请检查账号密码</span>
              )}
              {configForm.alist.token && (
                <div className="token-info">
                  <span className="token-label">Token（自动缓存，无需修改）:</span>
                  <code>{configForm.alist.token.substring(0, 20)}...</code>
                </div>
              )}
            </div>

            <div className="settings-section">
              <h3>上传配置</h3>
              <div className="form-group">
                <label>并发数:</label>
                <input
                  type="number"
                  min="1"
                  max="10"
                  value={configForm.upload.concurrency}
                  onChange={(e) => setConfigForm({
                    ...configForm,
                    upload: { ...configForm.upload, concurrency: parseInt(e.target.value) || 1 }
                  })}
                />
              </div>
              <div className="form-group">
                <label>最大重试次数:</label>
                <input
                  type="number"
                  min="0"
                  max="10"
                  value={configForm.upload.max_retries}
                  onChange={(e) => setConfigForm({
                    ...configForm,
                    upload: { ...configForm.upload, max_retries: parseInt(e.target.value) || 0 }
                  })}
                />
              </div>
              <div className="form-group checkbox-group">
                <input
                  type="checkbox"
                  id="asTask"
                  checked={configForm.upload.as_task}
                  onChange={(e) => setConfigForm({
                    ...configForm,
                    upload: { ...configForm.upload, as_task: e.target.checked }
                  })}
                />
                <label htmlFor="asTask">使用 Alist 后台任务模式</label>
              </div>
              <div className="form-group checkbox-group">
                <input
                  type="checkbox"
                  id="showProgress"
                  checked={configForm.upload.show_progress}
                  onChange={(e) => setConfigForm({
                    ...configForm,
                    upload: { ...configForm.upload, show_progress: e.target.checked }
                  })}
                />
                <label htmlFor="showProgress">显示上传进度</label>
              </div>
            </div>

            <div className="settings-section">
              <h3>历史记录</h3>
              <div className="form-group">
                <label>最大保留记录数:</label>
                <input
                  type="number"
                  min="10"
                  max="1000"
                  value={configForm.history.max_records}
                  onChange={(e) => setConfigForm({
                    ...configForm,
                    history: { ...configForm.history, max_records: parseInt(e.target.value) || 100 }
                  })}
                />
              </div>
            </div>

            <div className="settings-actions">
              <button onClick={handleSaveConfig} className="primary">
                保存配置
              </button>
            </div>
          </div>
        )}
      </main>
    </div>
  );
}

export default App;
