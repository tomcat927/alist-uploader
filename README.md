# Alist 上传管理器

基于 Tauri + React 的 Alist 网盘上传管理工具，支持队列管理、持久化存储和可配置的上传策略。

## 功能特性

- **队列管理**：添加、删除、清空上传队列
- **持久化存储**：程序重启后队列不丢失
- **单线程上传**：默认单任务上传，可配置并发数
- **失败重试**：自动重试机制，可配置重试次数
- **历史记录**：最多保留 100 条上传记录
- **As-Task 支持**：支持 Alist 后台任务模式
- **跨平台**：支持 Windows、macOS、Linux

## 技术栈

- **前端**：React 18 + TypeScript + Vite
- **状态管理**：Zustand
- **桌面框架**：Tauri 2.x
- **后端**：Rust
- **HTTP 客户端**：reqwest
- **异步运行时**：tokio

## 项目结构

```
alist-uploader/
├── src/                      # 前端源代码
│   ├── components/           # React 组件
│   ├── store/                # Zustand 状态管理
│   ├── types.ts              # TypeScript 类型定义
│   ├── App.tsx               # 主应用组件
│   └── main.tsx              # 入口文件
├── src-tauri/                # Tauri/Rust 后端
│   ├── src/
│   │   ├── commands/         # IPC 命令接口
│   │   ├── models/           # 数据模型
│   │   ├── services/         # 业务逻辑
│   │   ├── utils/            # 工具函数
│   │   ├── lib.rs            # 库入口
│   │   └── main.rs           # 程序入口
│   ├── capabilities/         # Tauri 权限配置
│   ├── Cargo.toml            # Rust 依赖
│   └── tauri.conf.json       # Tauri 配置
├── package.json              # Node.js 依赖
└── tsconfig.json             # TypeScript 配置
```

## 开发指南

### 环境要求

- **Node.js** >= 18
- **Rust** >= 1.70
- 操作系统依赖（Linux）：
  - `libwebkit2gtk-4.1-dev`
  - `libgtk-3-dev`
  - `libayatana-appindicator3-dev`
  - `libsoup-3.0-dev`
  - `libjavascriptcoregtk-4.1-dev`

### 安装依赖

```bash
# 安装 Node.js 依赖
npm install

# Rust 依赖会在首次编译时自动下载
```

### 开发模式

```bash
npm run tauri dev
```

### 构建发布版

```bash
npm run tauri build
```

构建完成后，安装包位于 `src-tauri/target/release/bundle/`

## 使用说明

### 添加上传任务

1. 在主界面点击"选择文件"按钮
2. 选择一个或多个要上传的文件
3. 指定 Alist 目标路径（如 `/dav/photos`）
4. 文件将添加到待上传队列

### 开始上传

1. 点击"开始上传"按钮
2. 程序会自动从队列中取出任务逐个上传
3. 上传完成的任务会自动移动到历史记录

### 暂停上传

点击"暂停"按钮可暂停当前上传，已添加到队列的任务会保留。

### 配置 Alist

1. 切换到"设置"标签页
2. 配置 Alist 服务地址（默认 `http://127.0.0.1:5244`）
3. 如需认证，填写 Token
4. 点击"测试连接"验证配置
5. 点击"保存配置"

### 上传配置

- **并发数**：同时上传的任务数量（默认 1）
- **最大重试次数**：失败后自动重试次数（默认 5）
- **As-Task 模式**：使用 Alist 后台任务功能（默认启用）
- **显示进度**：显示上传进度条（默认关闭）

## 数据存储

应用程序数据存储在：

- **Windows**：`%APPDATA%\alist-uploader\`
- **macOS**：`~/Library/Application Support/alist-uploader/`
- **Linux**：`~/.config/alist-uploader/`

存储文件：
- `queue.json` - 待上传队列
- `history.json` - 历史记录
- `config.json` - 应用配置

## Alist API 参考

本项目使用 Alist 的以下 API：

- `/api/me` - 获取当前用户信息
- `/api/fs/put` - 上传文件
- `/api/fs/list` - 列出目录内容
- `/api/admin/task/upload/list` - 获取上传任务列表

参考文档：https://alist-public.apifox.cn/

## 注意事项

1. **大文件上传**：建议启用 As-Task 模式，避免 HTTP 超时
2. **网络稳定性**：配置适当的重试次数以应对网络波动
3. **文件存在处理**：目前默认覆盖同名文件
4. **Alist 权限**：确保 Alist Token 有足够的上传权限

## 开发计划

- [ ] 拖拽添加文件支持
- [ ] 文件存在处理策略配置（覆盖/跳过/重命名/询问）
- [ ] 上传进度显示优化
- [ ] 批量操作支持
- [ ] 上传速度限制
- [ ] 日志查看功能

## 许可证

MIT License

## 贡献

欢迎提交 Issue 和 Pull Request！
# Manual trigger
