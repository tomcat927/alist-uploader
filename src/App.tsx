import { useEffect, useState, useRef } from 'react';
import { useAppStore } from './store/appStore';
import { open } from '@tauri-apps/plugin-dialog';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { invoke } from '@tauri-apps/api/core';
import { isPermissionGranted, requestPermission, sendNotification } from '@tauri-apps/plugin-notification';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { FolderPicker } from './components/FolderPicker';
import { DEFAULT_APP_CONFIG, normalizeAppConfig, type AppConfig } from './types';
import './App.css';

const FOUR_GB = 4 * 1024 * 1024 * 1024;

function App() {
  const {
    queue,
    history,
    config,
    configLoaded,
    isUploading,
    isLoading,
    alistConnected,
    alistServiceAvailable,
    alistChecking,
    isStopping,
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
    login,
  } = useAppStore();

  const [alistPath, setAlistPath] = useState('/');
  const [activeTab, setActiveTab] = useState<'queue' | 'history' | 'settings'>('queue');
  const [configForm, setConfigForm] = useState<AppConfig>(DEFAULT_APP_CONFIG);
  const [connectionStatus, setConnectionStatus] = useState<'idle' | 'success' | 'error'>('idle');
  const [connectionMessage, setConnectionMessage] = useState('');
  const [uploadPathError, setUploadPathError] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [speedLimitCustomMode, setSpeedLimitCustomMode] = useState(false);
  const [speedLimitCustomText, setSpeedLimitCustomText] = useState('');
  const autoLoginRef = useRef(false);
  const configInitializedRef = useRef(false);
  const alistPathRef = useRef('/');
  const savePathTimerRef = useRef<number | null>(null);
  const notifiedTaskIds = useRef<Set<string>>(new Set());

  const normalizeAlistPath = (path: string) => {
    const trimmed = path.trim();
    if (!trimmed || trimmed === '/') return '/';
    const withPrefix = trimmed.startsWith('/') ? trimmed : `/${trimmed}`;
    return withPrefix.replace(/\/+$/, '') || '/';
  };

  const speedLimitToMbps = (bytesPerSec: number): number =>
    bytesPerSec === 0
      ? 0
      : Math.round(bytesPerSec / 125000 * 100) / 100;

  const isRootAlistPath = (path: string) => normalizeAlistPath(path) === '/';

  const getRootPathMessage = () => '请选择 Alist 中的具体目录后再添加或上传文件，根目录 / 仅用于浏览存储入口。';

  const ensureConcreteAlistPath = async (path: string, action: string) => {
    if (!isRootAlistPath(path)) {
      setUploadPathError('');
      return true;
    }

    const message = getRootPathMessage();
    setUploadPathError(message);
    await writeClientLog(`${action} 被拦截: target_path=/, reason=根目录不是具体上传目录`);
    window.alert(message);
    return false;
  };

  useEffect(() => {
    loadQueue();
    loadHistory();
    loadConfig();
    
    // 启动心跳检测
    startHealthCheck();
    
    // 监听文件拖拽事件
    let unlistenFn: (() => void) | null = null;

    const setupDragDrop = async () => {
      try {
        const unlisten = await getCurrentWebview().onDragDropEvent(async (event) => {
          if (event.payload.type === 'drop') {
            const paths = event.payload.paths;
            if (Array.isArray(paths)) {
              const targetPath = alistPathRef.current;
              if (!(await ensureConcreteAlistPath(targetPath, '拖拽添加文件'))) {
                return;
              }
              await writeClientLog(`拖拽添加文件: count=${paths.length}, target_path=${targetPath}`);
              for (const filePath of paths) {
                try {
                  const result = await addToFileQueue(filePath, targetPath);
                  if (result.warnings.length > 0) {
                    window.alert(result.warnings.join('\n'));
                    await writeClientLog(`拖拽添加文件触发大文件提示: warning_count=${result.warnings.length}`);
                  }
                  await writeClientLog(`拖拽文件已加入队列: file_path=${filePath}, target_path=${targetPath}`);
                } catch (error) {
                  const message = error instanceof Error ? error.message : String(error);
                  await writeClientLog(`拖拽文件加入队列失败: file_path=${filePath}, target_path=${targetPath}, error=${message}`);
                  window.alert(message);
                  console.error('Failed to add file:', error);
                }
              }
            }
          }
        });

        unlistenFn = unlisten;
      } catch (error) {
        console.error('Failed to setup drag drop:', error);
      }
    };

    setupDragDrop();

    return () => {
      stopHealthCheck();
      if (unlistenFn) {
        unlistenFn();
      }
      if (savePathTimerRef.current) {
        window.clearTimeout(savePathTimerRef.current);
      }
    };
  }, [loadQueue, loadHistory, loadConfig, startHealthCheck, stopHealthCheck, addToFileQueue]);

  useEffect(() => {
    const normalizedConfig = normalizeAppConfig(config);
    setConfigForm(normalizedConfig);
    // 判断当前上传限速值是否在预设选项中，否则启用自定义模式
    const mbps = speedLimitToMbps(normalizedConfig.upload.speed_limit);
    const presets = [0, 1, 2, 5, 10];
    setSpeedLimitCustomMode(!presets.includes(mbps));
    setSpeedLimitCustomText(mbps === 0 ? '' : String(mbps));

    if (configLoaded && !configInitializedRef.current) {
      const savedPath = normalizeAlistPath(normalizedConfig.upload.last_alist_path || '/');
      setAlistPath(savedPath);
      alistPathRef.current = savedPath;
      setUploadPathError(isRootAlistPath(savedPath) ? getRootPathMessage() : '');
      configInitializedRef.current = true;
      writeClientLog(`恢复上次上传目标目录: target_path=${savedPath}`);
    }
    
    // 自动登录逻辑 - 只执行一次
    const performAutoLogin = async () => {
      // 使用 ref 确保自动登录只执行一次
      if (autoLoginRef.current) {
        return;
      }
      
      if (configLoaded && normalizedConfig.alist.auto_login && 
          normalizedConfig.alist.username && 
          normalizedConfig.alist.password && 
          normalizedConfig.alist.base_url) {
        autoLoginRef.current = true;
        try {
          await writeClientLog(`自动登录: base_url=${normalizedConfig.alist.base_url}, username=${normalizedConfig.alist.username}`);
          await login(normalizedConfig.alist.base_url, normalizedConfig.alist.username, normalizedConfig.alist.password);
          await writeClientLog('自动登录成功');
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          await writeClientLog(`自动登录失败: ${message}`);
          console.error('Auto login failed:', error);
          // 登录失败后不重试，避免无限循环
        }
      }
    };

    performAutoLogin();
  }, [config, configLoaded]);

  useEffect(() => {
    if (!isUploading) return;

    const intervalId = window.setInterval(async () => {
      try {
        await loadQueue();
        await loadHistory();
        const latestQueue = useAppStore.getState().queue;
        const hasActiveTask = latestQueue.some(task => task.status === 'pending' || task.status === 'uploading');
        if (!hasActiveTask) {
          setIsUploading(false);
          await writeClientLog('上传队列已完成，前端停止上传状态');
        }

        // 检测新完成的任务，发送系统通知
        const notifyOnComplete = useAppStore.getState().config.upload.notify_on_complete;
        if (notifyOnComplete) {
          const perm = await isPermissionGranted();
          if (!perm) {
            await requestPermission();
          }
          for (const task of latestQueue) {
            if (task.status === 'completed' && !notifiedTaskIds.current.has(task.id)) {
              notifiedTaskIds.current.add(task.id);
              sendNotification({ title: '上传完成', body: `文件 ${task.file.name} 上传成功` });
            }
            if (task.status === 'failed' && !notifiedTaskIds.current.has(task.id)) {
              notifiedTaskIds.current.add(task.id);
              sendNotification({ title: '上传失败', body: `${task.file.name}: ${task.error || '未知错误'}` });
            }
          }
        }
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        await writeClientLog(`上传队列刷新失败: ${message}`);
      }
    }, 1000);

    return () => window.clearInterval(intervalId);
  }, [isUploading, loadQueue, loadHistory, setIsUploading]);

  const persistAlistPath = (path: string) => {
    const normalizedPath = normalizeAlistPath(path);
    setAlistPath(normalizedPath);
    alistPathRef.current = normalizedPath;
    setUploadPathError(isRootAlistPath(normalizedPath) ? getRootPathMessage() : '');
    setConfigForm(current => normalizeAppConfig({
      ...current,
      upload: { ...current.upload, last_alist_path: normalizedPath },
    }));

    if (savePathTimerRef.current) {
      window.clearTimeout(savePathTimerRef.current);
    }

    savePathTimerRef.current = window.setTimeout(async () => {
      const latestConfig = normalizeAppConfig(useAppStore.getState().config);
      const nextConfig = normalizeAppConfig({
        ...latestConfig,
        upload: { ...latestConfig.upload, last_alist_path: alistPathRef.current },
      });
      try {
        await writeClientLog(`保存上传目标目录: target_path=${alistPathRef.current}`);
        await saveConfig(nextConfig);
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        await writeClientLog(`保存上传目标目录失败: target_path=${alistPathRef.current}, error=${message}`);
      }
    }, 500);
  };

  const handleSelectFiles = async () => {
    try {
      if (!(await ensureConcreteAlistPath(alistPathRef.current, '选择文件'))) {
        return;
      }

      const selected = await open({
        multiple: true,
      });
      
      if (selected) {
        const files = Array.isArray(selected) ? selected : [selected];
        const targetPath = alistPathRef.current;
        await writeClientLog(`文件选择添加队列: count=${files.length}, target_path=${targetPath}`);
        const warnings: string[] = [];
        for (const file of files) {
          const result = await addToFileQueue(file, targetPath);
          warnings.push(...result.warnings);
          await writeClientLog(`选择文件已加入队列: file_path=${file}, target_path=${targetPath}`);
        }
        if (warnings.length > 0) {
          window.alert(warnings.join('\n'));
          await writeClientLog(`选择文件触发大文件提示: warning_count=${warnings.length}`);
        }
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      await writeClientLog(`选择文件加入队列失败: target_path=${alistPathRef.current}, error=${message}`);
      window.alert(message);
      console.error('Failed to select files:', error);
    }
  };

  const handleStartUpload = async () => {
    const rootTargetTask = queue.find(task => isRootAlistPath(task.alist_path));
    if (rootTargetTask) {
      const message = `队列中存在目标目录为 / 的任务：${rootTargetTask.file.name}。请删除该任务后选择具体目录重新添加。`;
      setUploadPathError(message);
      await writeClientLog(`开始上传被拦截: task_id=${rootTargetTask.id}, file=${rootTargetTask.file.name}, target_path=/`);
      window.alert(message);
      return;
    }

    await startUpload();
  };

  const handlePauseUpload = async () => {
    await pauseUpload();
  };

  const writeClientLog = async (message: string) => {
    try {
      console.log('[client_log]', message);
      await invoke('write_client_log', { message });
    } catch (error) {
      console.error('[client_log_failed]', error);
    }
  };

  const handleTestConnection = async () => {
    await writeClientLog(`点击测试连接: base_url=${configForm.alist.base_url}, username=${configForm.alist.username}, has_token=${Boolean(configForm.alist.token)}`);

    try {
      const success = await testConnection(configForm);
      setConnectionStatus(success ? 'success' : 'error');
      setConnectionMessage(success ? 'Token 有效，连接成功' : 'Token 无效或未登录，请先点击登录获取 Token');
      await writeClientLog(`测试连接完成: success=${success}`);
      setTimeout(() => setConnectionStatus('idle'), 5000);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setConnectionStatus('error');
      setConnectionMessage(message);
      await writeClientLog(`测试连接异常: ${message}`);
      setTimeout(() => setConnectionStatus('idle'), 5000);
    }
  };

  const handleSaveConfig = async () => {
    await saveConfig(normalizeAppConfig(configForm));
  };

  const handleTestNotification = async () => {
    const notification = configForm.upload.notification;
    if (!notification || !notification.enabled) {
      window.alert('请先启用失败通知');
      return;
    }
    if (!notification.webhook_url.trim()) {
      window.alert('请先填写 Webhook URL');
      return;
    }
    await writeClientLog(`点击测试通知: has_webhook=${Boolean(notification.webhook_url)}, channels=${notification.channels.join(',')}`);
    try {
      await invoke('test_notification', { config: notification });
      await writeClientLog('测试通知发送成功');
      window.alert('测试通知发送成功，请检查飞书机器人消息');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      await writeClientLog(`测试通知发送失败: ${message}`);
      window.alert(`测试通知发送失败: ${message}`);
    }
  };

  const handleCheckUpdate = async () => {
    await writeClientLog('点击检查更新');
    try {
      const update = await check();
      if (update) {
        if (window.confirm(`发现新版本 ${update.version}，是否下载安装？`)) {
          await update.downloadAndInstall((event) => {
            if (event.event === 'Finished') {
              writeClientLog('更新下载完成');
            }
          });
          await writeClientLog('更新已安装，即将重启');
          await relaunch();
        }
      } else {
        window.alert('已是最新版本');
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      await writeClientLog(`检查更新失败: ${message}`);
      window.alert(`检查更新失败: ${message}`);
    }
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

  const formatDuration = (seconds: number) => {
    if (seconds < 60) return `${seconds}s`;
    const m = Math.floor(seconds / 60);
    const s = seconds % 60;
    return s > 0 ? `${m}m ${s}s` : `${m}m`;
  };

  const formatSpeed = (bytesPerSec: number) => {
    if (bytesPerSec <= 0) return '';
    const units = ['B/s', 'KB/s', 'MB/s', 'GB/s'];
    let speed = bytesPerSec;
    let unitIndex = 0;
    while (speed >= 1024 && unitIndex < units.length - 1) {
      speed /= 1024;
      unitIndex++;
    }
    return `${speed.toFixed(2)} ${units[unitIndex]}`;
  };

  const totalUploadSpeed = queue
    .filter(t => t.status === 'uploading')
    .reduce((sum, t) => sum + (t.speed || 0), 0);

  const formatDateTime = (isoString?: string) => {
    if (!isoString) return '-';
    return new Date(isoString).toLocaleString('zh-CN');
  };

  const hasRootTargetInQueue = queue.some(task => isRootAlistPath(task.alist_path));

  if (isLoading) {
    return <div className="loading">加载中...</div>;
  }

  return (
    <div className="app">
      <header className="app-header">
        <h1>Alist 上传管理器</h1>
        <div className="header-status">
          <div className="alist-status">
            <span className={`status-dot ${alistChecking ? 'checking' : alistServiceAvailable && alistConnected ? 'connected' : alistServiceAvailable ? 'warning' : 'disconnected'}`} />
            <span className="status-label">
              {alistChecking ? '检测中...' : alistServiceAvailable && alistConnected ? 'Alist 已连接' : alistServiceAvailable ? 'Alist 已启动，未登录' : 'Alist 服务不可用'}
            </span>
          </div>
          <span className={`status-indicator ${isUploading ? 'active' : ''}`}>
            {isUploading && totalUploadSpeed > 0
              ? `上传中 ${formatSpeed(totalUploadSpeed)}`
              : isUploading
                ? '上传中'
                : '空闲'}
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
              <div className="target-path-panel">
                <div className="target-path-header">
                  <span className="target-step">先选择上传位置</span>
                  <button
                    type="button"
                    className="secondary small"
                    onClick={() => persistAlistPath('/')}
                    disabled={alistPath === '/'}
                  >
                    使用根目录
                  </button>
                </div>
                <div className="upload-path-input">
                  <label>目标目录:</label>
                  <FolderPicker
                    value={alistPath}
                    onChange={persistAlistPath}
                  />
                </div>
                <div className="target-path-hint">
                  之后选择或拖拽的文件都会加入此目录；已加入队列的任务保留各自目标路径。
                </div>
                {uploadPathError && (
                  <div className="target-path-error">
                    {uploadPathError}
                  </div>
                )}
                {hasRootTargetInQueue && (
                  <div className="target-path-error">
                    队列中存在目标目录为 / 的任务，请删除后选择具体目录重新添加。
                  </div>
                )}
              </div>
              <div className="toolbar-actions">
                <button onClick={handleSelectFiles}>
                  选择文件
                </button>
                {!isUploading ? (
                  <button 
                    onClick={handleStartUpload} 
                    disabled={queue.length === 0 || hasRootTargetInQueue}
                    className="primary"
                    title={hasRootTargetInQueue ? '队列中存在目标目录为 / 的任务，请删除后重新添加' : undefined}
                  >
                    开始上传
                  </button>
                ) : isStopping ? (
                  <button disabled className="warning disabled">
                    停止中...
                  </button>
                ) : (
                  <button onClick={handlePauseUpload} className="warning">
                    停止队列
                  </button>
                )}
                <button onClick={clearQueue} disabled={queue.length === 0 || isUploading}>
                  清空队列
                </button>
              </div>
              {isStopping && (
                <div className="stopping-notice">
                  等待当前文件上传完成后停止...
                </div>
              )}
            </div>

            <div className="queue-list">
              {queue.length === 0 ? (
                <div className="empty-state">
                  <p>队列为空</p>
                  <p>当前目标目录：{alistPath}</p>
                  <p>先选择目标目录，再拖拽文件或点击"选择文件"添加</p>
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
                        <td>
                          <span>{task.alist_path}</span>
                          {task.file.size > FOUR_GB && (
                            <span className="large-file-badge" title="该文件超过 4GB。大文件上传耗时较长，若 1 小时内未完成可能因 Token 过期导致失败。建议在上传带宽较好时上传，或先压缩/分卷处理。">
                              大文件风险
                            </span>
                          )}
                        </td>
                        <td>
                          <span className={`status-badge status-${task.status}`}>
                            {task.status === 'pending' && '等待中'}
                            {task.status === 'uploading' && `上传中 ${task.progress}%`}
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
                        <td>{task.duration ? formatDuration(task.duration) : '-'}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          </div>
        )}

        {activeTab === 'settings' && (
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
                <div className="password-input-wrapper">
                  <input
                    type={showPassword ? "text" : "password"}
                    value={configForm.alist.password}
                    onChange={(e) => setConfigForm({
                      ...configForm,
                      alist: { ...configForm.alist, password: e.target.value }
                    })}
                    placeholder="填写你的 Alist 密码"
                  />
                  <button
                    type="button"
                    className="password-toggle"
                    onClick={() => setShowPassword(!showPassword)}
                    aria-label={showPassword ? '隐藏密码' : '显示密码'}
                  >
                    <span className={`eye-icon ${showPassword ? 'visible' : ''}`} />
                  </button>
                </div>
              </div>
              <div className="form-group checkbox-group">
                <input
                  type="checkbox"
                  id="autoLogin"
                  checked={configForm.alist.auto_login}
                  onChange={(e) => setConfigForm({
                    ...configForm,
                    alist: { ...configForm.alist, auto_login: e.target.checked }
                  })}
                />
                <label htmlFor="autoLogin">启动时自动登录</label>
              </div>
              <div className="toolbar-actions">
                <button 
                  onClick={async () => {
                    await writeClientLog(`点击登录获取 Token: base_url=${configForm.alist.base_url}, username=${configForm.alist.username}, password_length=${configForm.alist.password.length}`);

                    try {
                      await useAppStore.getState().login(
                        configForm.alist.base_url,
                        configForm.alist.username,
                        configForm.alist.password
                      );
                      setConnectionStatus('success');
                      setConnectionMessage('登录成功，Token 已保存');
                      await writeClientLog('登录获取 Token 成功');
                      setTimeout(() => setConnectionStatus('idle'), 5000);
                    } catch (error) {
                      const message = error instanceof Error ? error.message : String(error);
                      setConnectionStatus('error');
                      setConnectionMessage(message);
                      await writeClientLog(`登录获取 Token 失败: ${message}`);
                      setTimeout(() => setConnectionStatus('idle'), 5000);
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
                  {connectionMessage || (configForm.alist.token ? 'Token 已缓存' : '连接成功')}
                </span>
              )}
              {connectionStatus === 'error' && (
                <span className="test-result error">{connectionMessage || '认证失败，请检查账号密码'}</span>
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
              <div className="form-group">
                <label>上传限速 Mbps:</label>
                <div className="speed-limit-control">
                  <select
                    value={speedLimitCustomMode ? -1 : (() => {
                      const mbps = speedLimitToMbps(configForm.upload.speed_limit);
                      const presets = [0, 1, 2, 5, 10];
                      return presets.includes(mbps) ? mbps : -1;
                    })()}
                    onChange={(e) => {
                      const val = parseFloat(e.target.value);
                      if (val === -1) {
                        if (!speedLimitCustomMode) {
                          const mbps = speedLimitToMbps(configForm.upload.speed_limit);
                          setSpeedLimitCustomText(mbps === 0 ? '' : String(mbps));
                        }
                        setSpeedLimitCustomMode(true);
                        return;
                      }
                      setSpeedLimitCustomMode(false);
                      setSpeedLimitCustomText('');
                      const bytesPerSec = val === 0 ? 0 : Math.round(val * 125000);
                      setConfigForm({ ...configForm, upload: { ...configForm.upload, speed_limit: bytesPerSec } });
                    }}
                  >
                    <option value={0}>不限速</option>
                    <option value={1}>1 Mbps</option>
                    <option value={2}>2 Mbps</option>
                    <option value={5}>5 Mbps</option>
                    <option value={10}>10 Mbps</option>
                    <option value={-1}>自定义</option>
                  </select>
                  {speedLimitCustomMode && (
                    <input
                      type="number"
                      min="0"
                      max="1000"
                      step="0.1"
                      value={speedLimitCustomText}
                      onChange={(e) => {
                        setSpeedLimitCustomText(e.target.value);
                        const v = parseFloat(e.target.value);
                        const bytesPerSec = Number.isFinite(v) && v > 0 ? Math.round(v * 125000) : 0;
                        setConfigForm({ ...configForm, upload: { ...configForm.upload, speed_limit: bytesPerSec } });
                      }}
                    />
                  )}
                </div>
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

              <div className="form-group checkbox-group">
                <input
                  type="checkbox"
                  id="notifyOnComplete"
                  checked={configForm.upload.notify_on_complete}
                  onChange={(e) => setConfigForm({
                    ...configForm,
                    upload: { ...configForm.upload, notify_on_complete: e.target.checked }
                  })}
                />
                <label htmlFor="notifyOnComplete">上传完成后发送系统通知</label>
              </div>

              <div className="form-group checkbox-group">
                <input
                  type="checkbox"
                  id="blockFilesOver5gb"
                  checked={configForm.upload.block_files_over_5gb}
                  onChange={(e) => setConfigForm({
                    ...configForm,
                    upload: { ...configForm.upload, block_files_over_5gb: e.target.checked }
                  })}
                />
                <label htmlFor="blockFilesOver5gb">拦截超过 5GB 的单个文件（适用于 115 网盘非会员限制）</label>
              </div>

              <div className="form-group checkbox-group">
                <input
                  type="checkbox"
                  id="warnFilesOver4gb"
                  checked={configForm.upload.warn_files_over_4gb}
                  onChange={(e) => setConfigForm({
                    ...configForm,
                    upload: { ...configForm.upload, warn_files_over_4gb: e.target.checked }
                  })}
                />
                <label htmlFor="warnFilesOver4gb">对超过 4GB 的文件显示上传失败风险提示</label>
              </div>

              <div className="large-file-notice">
                115 网盘非会员单个文件最大支持 5GB；超过 4GB 的文件上传耗时较长，若 1 小时内未完成可能因 Token 过期导致失败。
              </div>
              
              <div className="form-group radio-group">
                <label>上传方式:</label>
                <div className="radio-options">
                  <label className="radio-label">
                    <input
                      type="radio"
                      name="uploadMethod"
                      value="stream"
                      checked={configForm.upload.upload_method === 'stream'}
                      onChange={(e) => setConfigForm({
                        ...configForm,
                        upload: { ...configForm.upload, upload_method: e.target.value }
                      })}
                    />
                    Stream（流式上传）
                  </label>
                  <label className="radio-label">
                    <input
                      type="radio"
                      name="uploadMethod"
                      value="form"
                      checked={configForm.upload.upload_method === 'form'}
                      onChange={(e) => setConfigForm({
                        ...configForm,
                        upload: { ...configForm.upload, upload_method: e.target.value }
                      })}
                    />
                    Form（表单上传）
                  </label>
                </div>
              </div>
              
              <div className="form-group checkbox-group">
                <input
                  type="checkbox"
                  id="enableNotification"
                  checked={configForm.upload.notification?.enabled || false}
                  onChange={(e) => setConfigForm({
                    ...configForm,
                    upload: { 
                      ...configForm.upload, 
                      notification: {
                        enabled: e.target.checked,
                        webhook_url: configForm.upload.notification?.webhook_url || "",
                        channels: configForm.upload.notification?.channels || ["feishu"],
                      }
                    }
                  })}
                />
                <label htmlFor="enableNotification">启用失败通知</label>
              </div>
              
              {(configForm.upload.notification?.enabled || false) && (
                <div className="notification-settings">
                  <div className="form-group">
                    <label>Webhook URL:</label>
                    <input
                      type="text"
                      placeholder="https://open.feishu.cn/open-apis/bot/v2/hook/..."
                      value={configForm.upload.notification?.webhook_url || ""}
                      onChange={(e) => setConfigForm({
                        ...configForm,
                        upload: { 
                          ...configForm.upload, 
                          notification: {
                            enabled: configForm.upload.notification?.enabled || false,
                            webhook_url: e.target.value,
                            channels: configForm.upload.notification?.channels || ["feishu"],
                          }
                        }
                      })}
                    />
                  </div>
                  <div className="notification-notice">
                    ℹ️ 当文件上传失败超过重试阈值时，将发送通知并停止队列
                  </div>
                  <div className="settings-actions">
                    <button onClick={handleTestNotification} className="secondary small">
                      发送测试通知
                    </button>
                  </div>
                </div>
              )}
              
              <div className="form-group checkbox-group">
                <input
                  type="checkbox"
                  id="enableSchedule"
                  checked={configForm.upload.schedule?.enabled || false}
                  onChange={(e) => setConfigForm({
                    ...configForm,
                    upload: { 
                      ...configForm.upload, 
                      schedule: {
                        enabled: e.target.checked,
                        start_time: configForm.upload.schedule?.start_time || "03:00",
                        end_time: configForm.upload.schedule?.end_time || "07:00",
                      }
                    }
                  })}
                />
                <label htmlFor="enableSchedule">启用定时上传</label>
              </div>
              
              {(configForm.upload.schedule?.enabled || false) && (
                <div className="schedule-settings">
                  <div className="form-group">
                    <label>开始时间:</label>
                    <input
                      type="time"
                      value={configForm.upload.schedule?.start_time || "03:00"}
                      onChange={(e) => setConfigForm({
                        ...configForm,
                        upload: { 
                          ...configForm.upload, 
                          schedule: {
                            enabled: configForm.upload.schedule?.enabled || false,
                            start_time: e.target.value,
                            end_time: configForm.upload.schedule?.end_time || "07:00",
                          }
                        }
                      })}
                    />
                  </div>
                  <div className="form-group">
                    <label>结束时间:</label>
                    <input
                      type="time"
                      value={configForm.upload.schedule?.end_time || "07:00"}
                      onChange={(e) => setConfigForm({
                        ...configForm,
                        upload: { 
                          ...configForm.upload, 
                          schedule: {
                            enabled: configForm.upload.schedule?.enabled || false,
                            start_time: configForm.upload.schedule?.start_time || "03:00",
                            end_time: e.target.value,
                          }
                        }
                      })}
                    />
                  </div>
                  <div className="schedule-notice">
                    ℹ️ 到开始时间自动上传，到结束时间等待当前任务完成后停止
                  </div>
                </div>
              )}
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
              <button onClick={handleCheckUpdate} className="secondary">
                检查更新
              </button>
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
