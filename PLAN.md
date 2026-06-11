# clipboradsync 跨平台剪贴板同步工具 — 项目策划书

## 1. 项目概述

**clipboradsync** 是一款跨平台（Linux / Windows / macOS）的剪贴板同步工具，支持文本、图片、文件的实时同步。采用 Rust 编写，分服务端和客户端两个二进制。

### 核心目标

- 一端复制，所有设备自动同步
- 支持文本、图片、文件（不限大小，自动/手动模式控制）
- 两种部署模式：自建服务端（WS 实时推送）或 WebDAV 直连（无需中间服务器）
- 系统托盘常驻 + 全局热键，不干扰日常工作

## 2. 技术架构

```
┌─────────────────────────┐      WS 推送(通知)        ┌──────────────────────┐
│  客户端 A (Linux)       │ ─────────────────────────▶ │                      │
│  ┌───────────────────┐  │      HTTP 上传/下载        │   服务端 (可选)       │
│  │ clipboard         │  │ ◀────────────────────────▶ │  ┌────────────────┐  │
│  │ file_clipboard    │  │                           │  │ Axum HTTP/WS   │  │
│  │ server_backend    │  │                           │  │ SQLite 存储    │  │
│  │ webdav_backend    │  │      WebDAV PROPFIND      │  │ 文件系统存储   │  │
│  │ tray / hotkey     │  │ ◀────────────────────────▶ │  └────────────────┘  │
│  └───────────────────┘  │      (直连，无服务端)       └──────────────────────┘
└─────────────────────────┘
                                        ▲
                                        │
┌─────────────────────────┐             │
│  客户端 B (Windows)     │ ────────────┘
│  同上模块                │  WS 通知 / WebDAV 轮询
└─────────────────────────┘
```

### 通信协议

| 场景 | 协议 | 说明 |
|------|------|------|
| 实时通知 | WebSocket (WS) | 服务端广播新条目 ID 给所有客户端 |
| 数据上传 | HTTP POST /api/v1/entries | multipart/form-data，含设备和内容类型 |
| 数据下载 | HTTP GET /api/v1/entries/latest | 获取最新条目，返回 JSON |
| 文件下载 | HTTP GET /api/v1/entries/:id/file | 流式下载原始文件 |
| 历史拉取 | HTTP GET /api/v1/entries/since/:ts | 断连后增量同步 |
| WebDAV 存储 | PROPFIND / PUT / GET | 直接操作 WebDAV 目录 |

### 内容检测优先级

`剪贴板 → image > files > text → Empty`

- **图片**：arboard 读取 RGBA → image crate 编码为 PNG 上传
- **文件**：平台原生检测（xclip CF_HDROP NSPasteboard）→ 解析 file:// URI → 读取文件二进制上传
- **文本**：arboard 直接读取 UTF-8 文本

### 防循环机制

- 每个上传附带 `device_id`
- 客户端收到通知时检查 `device_id` 是否匹配自己，匹配则跳过下载
- 每条记录有唯一 ID，已下载的不重复下载

## 3. 项目结构

```
clipboradsync/
├── config.toml              # 客户端配置文件模板
├── server/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs          # Axum HTTP/WS 服务端 (550行)
│       └── config.rs        # 服务端配置加载 (50行)
├── client/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs          # 客户端入口、事件循环、上传/下载逻辑 (320行)
│       ├── clipboard.rs     # 文本/图片剪贴板读写 (65行)
│       ├── file_clipboard.rs # 三平台文件检测 + 写入 (200行)
│       ├── server_backend.rs # WS 长连接 + HTTP API + 轮询回退 (330行)
│       ├── webdav_backend.rs # WebDAV PROPFIND/PUT/GET (200行)
│       ├── config.rs        # 客户端配置加载 (154行)
│       ├── tray.rs          # 系统托盘 (80行)
│       └── hotkey.rs        # 全局热键 (45行)
└── PLAN.md                  # 本策划书
```

## 4. 功能清单

### ✅ 已完成

| 模块 | 功能 | 状态 |
|------|------|------|
| 服务端 | Axum HTTP REST API (上传/查询/下载/历史) | ✅ |
| 服务端 | WebSocket 广播新条目 | ✅ |
| 服务端 | 文件磁盘存储 | ✅ |
| 服务端 | SQLite 元数据 | ✅ |
| 服务端 | 历史记录自动清理（按天，后台每小时） | ✅ |
| 服务端 | 配置文件 + 环境变量覆盖 | ✅ |
| 客户端 | Server 模式：WS 连接 + HTTP 上传下载 | ✅ |
| 客户端 | WS 断连自动 HTTP 轮询回退 | ✅ |
| 客户端 | WebDAV 模式：PROPFIND 轮询 + PUT/GET | ✅ |
| 客户端 | 文本同步（双向） | ✅ |
| 客户端 | 图片同步（PNG 编解码） | ✅ |
| 客户端 | 文件同步（Linux xclip/wl-paste → file:// URI） | ✅ |
| 客户端 | 文件同步（Windows CF_HDROP，clipboard-win） | ✅ |
| 客户端 | 文件同步（macOS NSPasteboard，objc） | ✅ |
| 客户端 | 下载文件写回剪贴板（三平台文件引用） | ✅ |
| 客户端 | 系统托盘（Linux GTK，headless 安全降级） | ✅ |
| 客户端 | 全局热键 Ctrl+Shift+F1/F2 | ✅ |
| 客户端 | 自动/手动模式切换 | ✅ |
| 客户端 | 配置文件（多路径搜索） | ✅ |

## 5. 配置说明

### 客户端 config.toml

```toml
backend = "server"  # "server" | "webdav"

[server]
url = "ws://127.0.0.1:8765"
token = "default-token"

[webdav]
url = "https://nextcloud.example.com/remote.php/dav/files/user/clipboard/"
username = "user"
password = "pass"

[clipboard]
auto_sync_max_size = 52428800  # 50MB，超过此大小自动模式不上传
auto_check_interval_sec = 1    # 剪贴板检测间隔（秒）
temp_dir = "/tmp/clipboard-sync"  # 下载文件临时目录

[hotkeys]
upload = "Ctrl+Shift+F1"
download = "Ctrl+Shift+F2"
```

搜索路径：`./config.toml` → `~/.config/clipboard-sync/config.toml` → `/etc/clipboard-sync/config.toml`

### 服务端配置

支持配置文件 + 环境变量覆盖（优先级更高）：

| 配置项 | 配置文件字段 | 环境变量 |
|--------|------------|---------|
| 端口 | `port` | `CLIPSYNC_PORT` |
| 鉴权令牌 | `token` | `CLIPSYNC_TOKEN` |
| 存储路径 | `storage_path` | `CLIPSYNC_STORAGE_PATH` |
| 历史保留天数 | `max_history_days` | `CLIPSYNC_MAX_HISTORY_DAYS` |

搜索路径：`./server/config.toml` → `./config.toml` → `/etc/clipboard-sync/config.toml`

## 6. 部署方案

### 方案 A：自建服务端（推荐）

```
[服务端] 云服务器/VPS，有公网 IP
    └── 运行 clipboard-sync-server（端口 8765）
[客户端] 每台设备运行 clipboard-sync-client
    └── config.toml 中 server.url = "ws://公网IP:8765"
```

建议使用反向代理（Nginx/Caddy）加 TLS。

### 方案 B：WebDAV 直连

```
[客户端] 每台设备运行 clipboard-sync-client
    └── config.toml 中 backend = "webdav"
    └── 所有设备共用同一个 WebDAV 目录（NextCloud / ownCloud / 任意 WebDAV）
```

无需中间服务端，但同步延迟取决于轮询间隔（默认 2 秒）。

## 7. 限制与注意事项

- **无端到端加密**：数据在传输过程中依赖 TLS，服务端存储未加密。敏感数据请使用 TLS 并限制服务端访问。
- **文件剪贴板写入**：下载的文件保存到 `temp_dir` 后，系统剪贴板中存放的是本地文件路径引用，并非文件内容本身。
- **Linux 依赖**：xclip（X11）或 wl-clipboard（Wayland）需要安装在系统中。
- **headless 环境**：无显示器时系统托盘和全局热键会安全降级，不影响核心同步功能。
- **WebDAV 文件类型识别**：按文件名前缀判断（`text_`、`image_`），自定义命名的文件会被归类为 `file` 类型。
- **大文件处理**：不设硬上限，但大于 `auto_sync_max_size` 时自动模式不处理，需手动热键上传/下载。

## 8. 技术栈

| 技术 | 用途 |
|------|------|
| Rust 2021 Edition | 开发语言 |
| Axum 0.7 | 服务端 HTTP/WS 框架 |
| SQLite (rusqlite) | 元数据存储 |
| Tokio | 异步运行时 |
| reqwest | HTTP 客户端 |
| tokio-tungstenite | WebSocket 客户端 |
| arboard | 文本/图片剪贴板 |
| image crate | PNG 编解码 |
| tray-icon | 系统托盘 |
| rdev | 全局热键监听 |
| clipboard-win | Windows CF_HDROP |
| objc | macOS NSPasteboard |
| Serde + Toml | 配置序列化 |
