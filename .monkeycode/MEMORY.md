# 用户指令记忆

## 格式

### 用户指令条目
[用户指令摘要]
- Date: [YYYY-MM-DD]
- Context: [提及的场景或时间]
- Instructions:
  - [逐行描述]

### 项目知识条目
[项目知识摘要]
- Date: [YYYY-MM-DD]
- Context: Agent 在执行 [具体任务描述] 时发现
- Category: [运维部署|构建方法|测试方法|排错调试|工作流协作|环境配置]
- Instructions:
  - [逐行描述]

## 条目

### 配置文件向后兼容规则
- Date: 2026-06-12
- Context: Agent 在修复旧版配置文件加载失败时发现
- Category: 排错调试
- Instructions:
  - Rust 结构体新增字段时必须在字段上加 `#[serde(default)]` 或 `#[serde(default = "fn_name")]`，否则旧用户配置文件反序列化失败
  - 反序列化失败时前端 catch 后回退到 DEFAULT_APP_CONFIG，所有凭据丢失
  - 登录成功但保存失败的根因通常是 save_config 内部反序列化旧配置文件失败

### 构建与打包
- Date: 2026-06-12
- Context: Agent 在开发 Tauri 项目时发现
- Category: 构建方法
- Instructions:
  - 前端构建：`npm run build`（tsc + vite build）
  - 完整 Tauri 打包：`npm run tauri build`
  - 打包配置在 `src-tauri/tauri.conf.json`，Windows 只生成 NSIS (EXE)，不生成 MSI
  - GitHub Actions 构建配置在 `.github/workflows/build-windows.yml`

### 项目技术栈
- Date: 2026-06-12
- Context: Agent 在分析项目结构时发现
- Category: 环境配置
- Instructions:
  - 前端：React 19 + TypeScript + Vite 6 + Zustand 状态管理
  - 后端：Tauri v2 (Rust)，废弃 v1 API（如 tauri://file-drop）
  - 文件拖拽使用 `getCurrentWebview().onDragDropEvent()` 替代旧的 `listen('tauri://file-drop')`
  - Tauri v2 插件：dialog, fs

### 上传方式
- Date: 2026-06-12
- Context: Agent 在实现上传功能时发现
- Category: 环境配置
- Instructions:
  - 两种上传方式：Stream（/api/fs/put，原始 body，默认）和 Form（/api/fs/form，multipart）
  - `As-Task` 和 `File-Path` 必须作为 HTTP 请求头传递，不是 query 参数
  - 流式上传使用 `tokio_util::io::ReaderStream` 避免大文件内存暴涨
