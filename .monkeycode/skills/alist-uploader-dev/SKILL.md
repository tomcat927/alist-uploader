---
name: alist-uploader-dev
description: Alist Uploader 项目开发技能包。包含项目技术栈、Alist API 接口规范、开发规范（日志、配置兼容、错误处理）、Tauri v2 开发要点。开发此项目新功能时必须加载。
---

# Alist Uploader 项目开发技能包

Tauri v2 桌面应用，用于将本地文件批量上传至 Alist 服务。

## 技术栈

| 层 | 技术 |
|---|---|
| 前端 | React 19 + TypeScript + Vite 6 + Zustand |
| 后端 | Tauri v2 (Rust) |
| HTTP | reqwest (Rust 侧) |
| 序列化 | serde / serde_json |
| 文件遍历 | walkdir |
| 流式上传 | tokio-util::io::ReaderStream |

## 项目结构

```
src/                          # React 前端
  App.tsx                     # 主入口 + 设置页 UI + 拖拽监听
  types.ts                    # 类型定义 + 默认配置 + normalizeAppConfig
  store/appStore.ts           # Zustand 状态管理（队列、配置、上传控制）
  hooks/useFileDrop.ts        # Tauri v2 文件拖拽 hook
  components/                 # UI 组件
src-tauri/src/
  lib.rs                      # Tauri 入口，注册命令，panic hook
  main.rs                     # Rust main
  commands/mod.rs             # 所有 Tauri 命令（前后端桥接）
  models/mod.rs               # 数据结构（AppConfig, UploadConfig 等）
  services/
    alist_client.rs           # Alist HTTP API 客户端
    upload_scheduler.rs       # 上传调度器（并发控制）
    queue_manager.rs          # 队列管理
    schedule_manager.rs       # 定时任务
  utils/
    log.rs                    # 统一日志模块
    storage.rs                # JSON 文件读写（config/queue/history）
    fs.rs                     # 文件系统工具（遍历目录等）
```

## Alist API 参考

所有 Alist API 接口均基于 HTTP，认证方式为在 `Authorization` 请求头直接传递 token。

### 通用响应格式

```json
{
  "code": 200,
  "message": "success",
  "data": { ... }
}
```

后端 Rust 模型对应 `AlistResponse<T>`，解析时检查 `code == 200` 判断成功。

### API 列表

#### 登录
```
POST /api/auth/login
Body: { "username": "...", "password": "..." }
Response: { "code": 200, "data": { "token": "..." } }
```
- 超时 10 秒
- 返回的 token 直接作为 Authorization header 值使用

#### 获取当前用户
```
GET /api/me
Headers: Authorization: <token>
Response: { "code": 200, "data": { "id": 1, "username": "..." } }
```
- 用于验证 token 有效性（`test_connection`）

#### 健康检查
```
GET /ping
```
- 不携带 token，仅检查服务可达性

#### 列出目录
```
POST /api/fs/list
Headers: Authorization: <token>
Body: { "path": "/target/path" }
Response: { "code": 200, "data": { "content": [ { "name": "...", "size": 0, "is_dir": false } ] } }
```
- 注意：接口用 POST 而非 GET，body 传 JSON

#### 流式上传（推荐/默认）
```
PUT /api/fs/put
Headers:
  Authorization: <token>
  File-Path: /target/path/filename.ext
  As-Task: true           （可选，后台任务模式）
  Content-Length: <size>
Body: 原始二进制文件流
```
- 使用 `tokio_util::io::ReaderStream` 流式读取，避免大文件内存暴涨
- `File-Path` 是请求头，不是 query 参数
- `As-Task` 是请求头，不是 query 参数

#### 表单上传
```
PUT /api/fs/form
Headers:
  Authorization: <token>
  File-Path: /target/path/filename.ext
  As-Task: true           （可选）
Body: multipart/form-data, field name = "file"
```
- 同样支持流式 body（`multipart::Part::stream_with_length`）

#### 查询上传任务
```
GET /api/admin/task/upload/list?page=1&per_page=100
Headers: Authorization: <token>
Response: { "code": 200, "data": { "tasks": [...], "total": 0 } }
```
- `state` 字段：0=pending, 1=running, 2=succeeded, 3=cancelled, 4=error

## 开发规范

### 1. 日志规范（最高优先级）

**新功能必须添加充分日志**，方便线上问题定位。日志等级使用：

```rust
// 关键流程节点
crate::utils::log::log("开始某某操作: param1=xxx, param2=xxx");

// 外部请求/响应
crate::utils::log::log(&format!("请求 Alist: url={}, method=POST", url));
crate::utils::log::log(&format!("Alist 响应: status={}, code={}", status, code));

// 错误时的上下文（包含原始响应体）
crate::utils::log::log(&format!("解析失败: error={}, body={}", err, response_text));

// 前端事件也通过 write_client_log 传递
crate::utils::log::log(&format!("前端事件: 点击登录获取 Token: base_url=..."));
```

**日志函数参考** (`src-tauri/src/utils/log.rs`):
| 函数 | 用途 |
|---|---|
| `log(msg)` | 通用日志（最常用） |
| `log_error(msg, err)` | 错误日志 |
| `log_debug(msg)` | 调试日志 |
| `log_info(msg)` | 信息日志 |
| `log_warn(msg)` | 警告日志 |
| `log_request(method, url, status, duration)` | HTTP 请求日志 |

日志输出位置：
- `{data_local_dir}/alist-uploader/debug.log` — 滚动日志
- `{data_local_dir}/alist-uploader/logs/alist-YYYY-MM-DD.log` — 按日归档
- `{data_local_dir}/alist-uploader/startup.log` — 启动日志
- `{data_local_dir}/alist-uploader/panic.log` — panic 日志

### 2. 配置向后兼容规则

Rust 结构体新增字段时，**必须**添加 serde default 注解：

```rust
// 正确示例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadConfig {
    pub concurrency: u8,
    #[serde(default = "default_upload_method")]
    pub upload_method: String,              // 新增字段，兼容旧配置
}

fn default_upload_method() -> String {
    "stream".to_string()
}
```

前端 TypeScript 端通过 `normalizeAppConfig` 做默认值合并。

**教训**：忘记添加会导致旧用户配置文件反序列化失败 → 整份配置丢失 → 回退到默认值 → 账号密码全丢。

### 3. Tauri v2 开发要点

- **文件拖拽**：使用 `getCurrentWebview().onDragDropEvent()`，废弃 v1 的 `listen('tauri://file-drop')`
- **命令注册**：在 `src-tauri/src/lib.rs` 的 `invoke_handler` 中注册
- **状态管理**：通过 `tauri::Builder::default().manage(queue_manager)` 注入，命令中 `State<'_, QueueManager>` 获取
- **插件**：dialog (文件选择)、fs (文件系统)、opener (打开 URL)
- **前端调用**：使用 `@tauri-apps/api` 的 `invoke<T>('command_name', { args })`
- **自动登录防循环**：useEffect 监听 config 变化时，用 `useRef(false)` 防止循环触发

### 4. 错误处理模式

```rust
// 统一错误类型
#[derive(Error, Debug)]
pub enum AlistError {
    #[error("HTTP 请求失败：{0}")]
    Request(#[from] reqwest::Error),
    #[error("Alist API 错误：{0}")]
    Api(String),
}

// 命令层统一 Result<T, String>
#[tauri::command]
pub async fn my_command() -> Result<MyData, String> {
    // ...
    Ok(data)
}
```

### 5. 存储层

配置文件存储在 `{config_dir}/alist-uploader/`：
- `config.json` — 应用配置（Alist 连接信息、上传设置）
- `queue.json` — 上传队列
- `history.json` — 上传历史

使用 `Storage::read_json<T>()` 和 `Storage::write_json()` 读写。不存在时返回 `T::default()`。

## 构建与运行

```bash
# 开发模式
npm run tauri dev

# 前端构建
npm run build

# 完整打包（Windows EXE）
npm run tauri build

# GitHub Actions 构建
# 配置文件：.github/workflows/build-windows.yml
# 目前只生成 NSIS EXE，不生成 MSI
```

## 开发新功能检查清单

- [ ] Rust 结构体新增字段是否添加了 `#[serde(default)]`？
- [ ] 关键流程是否添加了充分的 `log()` 调用？
- [ ] HTTP 请求是否记录了 url、status、响应体（失败时）？
- [ ] 错误信息是否包含足够的上下文（参数值、响应内容）？
- [ ] Tauri 命令是否在 `lib.rs` 的 `invoke_handler` 中注册？
- [ ] 前端新增字段是否在 `types.ts` 的默认值中定义？
- [ ] 是否使用 Tauri v2 API（非 v1 旧 API）？
