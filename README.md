# ClipSync

> 自托管、轻量级跨设备剪贴板同步工具。一次复制，所有设备立即可用。

## 关于

在日常开发和使用中，经常需要在电脑、手机之间传递文本、图片或文件——发微信、发邮件、开共享文件夹都太慢了。ClipSync 通过自建服务端实现剪贴板实时同步，不依赖任何第三方云服务，数据完全自主可控。

当你在一台设备上复制内容后，ClipSync 会立刻推送到所有已连接的设备，无需任何额外操作。支持 **文本**、**图片**和**文件**三类内容的同步，自动模式下完全无感，也可通过热键手动控制上传/下载。

**核心特点：**

- **自托管**——服务端部署在你自己的 Linux 服务器上，数据不外泄
- **实时同步**——基于 WebSocket 长连接，复制即推送，延迟毫秒级
- **轻量无感**——Windows 客户端仅托盘图标运行，无窗口、无控制台，不打扰工作
- **离线不丢**——设备离线期间的内容会在上线时自动推送，不会遗漏
- **跨平台**——当前支持 Windows 客户端 + Linux 服务端，Android 客户端规划中

---

## 架构

```
┌──────────────┐                         ┌──────────────┐
│   Windows     │◄─── WebSocket ◄──────►│   Server      │
│   (客户端)    │◄─── REST API ◄──────►│   (Linux)     │
└──────────────┘                         └──────┬───────┘
                                                │
                                       ┌────────▼───────┐
                                       │   Android       │
                                       │   (规划中)       │
                                       └─────────────────┘
```

| 组件 | 技术栈 | 说明 |
|------|--------|------|
| **Server** | Rust + Axum + SQLite | 剪贴板中转服务，WebSocket 实时推送 + REST API |
| **Windows Client** | Rust + tray-icon + windows-sys | 托盘图标运行，全局热键，无窗口 |
| **Android Client** | Kotlin（规划中） | 前台自动同步，后台手动同步 |

---

## 功能

### 内容同步

| 类型 | 传输方式 | 说明 |
|------|----------|------|
| 文本 | WebSocket 内联 | 小于 `ws_inline_max_bytes`（默认 1MB）的文本直接经 WS 推送 |
| 图片 | HTTP 上传 + WS 通知 | 转为 PNG 后上传至 `/file/{name}`，WS 广播元数据 |
| 文件 | HTTP 上传 + WS 通知 | 读取磁盘内容，上传至服务端 |
| 超大内容 | 手动同步 | 超过 `auto_sync_max_bytes`（默认 10MB）需手动触发上传/下载 |

### 同步模式

| 模式 | 触发方式 | 说明 |
|------|----------|------|
| **自动同步** | 检测到剪贴板变化 | 全局开关，可在托盘菜单一键切换。自动模式下推送到所有在线设备并接收远端内容 |
| **手动上传** | 托盘菜单 / 热键 | 立即上传当前剪贴板内容 |
| **手动下载** | 托盘菜单 / 热键 | 从服务端拉取最新内容并写入剪贴板 |

### 全局热键

| 热键 | 功能 |
|------|------|
| `Ctrl + Shift + C` | 复制当前内容并立即同步到所有设备 |
| `Ctrl + Shift + V` | 从服务端拉取最新内容并粘贴 |
| `Ctrl + Alt + V` | 切换自动同步开关 |

热键组合可在 `config.toml` 中自定义。

### 托盘菜单

连接状态下托盘菜单实时展示：

- **连接状态**（彩色圆点：绿/黄/红）+ 最近同步时间 + 来源设备名
- **Upload** / **Download** ——手动同步
- **Auto-Sync** ——勾选开关自动同步
- **Settings** ——子菜单：编辑配置文件 / 打开配置目录
- **Launch at Startup** ——开机自启开关
- **Restart** ——重启客户端
- **Open Log** ——打开日志文件
- **Quit** ——退出

### 单实例保护

同时只能运行一个 ClipSync 客户端实例。重复启动会自动激活已有实例。

---

## 协议

### WebSocket 消息

客户端与服务端通过 WebSocket 通信，消息格式为 JSON，通过 `type` 字段区分：

**客户端 → 服务端：**

| 消息类型 | 用途 |
|----------|------|
| `Auth` | 携带 token、device_id、设备名进行身份认证 |
| `ClipSync` | 通知服务端有新内容，附 `ProfileDto` 载荷 |
| `GetLatest` | 请求服务端返回最新的剪贴板内容 |

**服务端 → 客户端：**

| 消息类型 | 用途 |
|----------|------|
| `AuthOk` / `AuthError` | 认证结果 |
| `ClipBroadcast` | 广播新内容到所有在线客户端 |
| `Backlog` | 下发离线期间遗漏的内容列表 |
| `LatestProfile` | 返回最新的剪贴板条目（含来源设备 ID 和时间） |

**心跳：** 客户端每 30 秒发送 WebSocket Ping 帧，防止空闲连接被中断。

### REST API

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET` / `PUT` | `/SyncClipboard.json` | 获取/更新最新剪贴板内容（需要认证） |
| `GET` / `PUT` | `/file/{filename}` | 上传/下载二进制文件（需要认证） |
| `GET` | `/profile/latest` | 获取最新 Profile（含来源设备 ID） |
| `GET` | `/health` | 健康检查（无需认证） |
| `GET` | `/api/time` | 获取服务端时间戳 |

**认证方式：** HTTP Basic Auth。用户名为空，密码为 `token`。服务端对 `/SyncClipboard.json` 和 `/file/*` 接口进行认证保护。

### ProfileDto 结构

```json
{
    "type": "Text | Image | File",
    "hash": "SHA-256 hex string",
    "text": "文本内容（仅 Text 类型）",
    "has_data": true,
    "data_name": "文件名（Image/File 类型）",
    "size": 12345
}
```

---

## 快速开始

### 服务端部署

**环境要求：** Linux（任何发行版）、Rust 工具链

```bash
# 克隆仓库
git clone https://github.com/CHD073/ClipSync.git
cd ClipSync/clipsync-server

# 构建
cargo build --release

# 运行（可通过环境变量配置）
export PORT=8765
export SYNC_TOKEN="your_secret_token"
export STORAGE_PATH="./data"
export MAX_HISTORY_DAYS=7
./target/release/clipsync-server
```

使用 systemd 长期运行：

```ini
# /etc/systemd/system/clipsync.service
[Unit]
Description=ClipSync Server
After=network.target

[Service]
Type=simple
ExecStart=/opt/clipsync/clipsync-server
Environment=PORT=8765
Environment=SYNC_TOKEN=my_token
Environment=STORAGE_PATH=/var/lib/clipsync
Restart=always

[Install]
WantedBy=multi-user.target
```

### Windows 客户端

1. 构建或下载 `clipsync.exe`
2. 同目录放置 `config.toml`，或首次运行自动生成默认配置
3. 双击运行，托盘图标出现即表示已连接

首次运行会自动生成 `device_id`（UUID v4）并写入配置。

---

## 配置参考

### 服务端（环境变量）

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `PORT` | `8765` | 监听端口 |
| `SYNC_TOKEN` | `clipsync` | 认证令牌 |
| `STORAGE_PATH` | `./data` | 数据（SQLite 数据库 + 上传文件）存储路径 |
| `MAX_HISTORY_DAYS` | `7` | 历史记录保留天数 |

### 客户端（config.toml）

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `server_url` | string | — | 服务端地址，形如 `http://ip:8765` |
| `token` | string | — | 与服务端 `SYNC_TOKEN` 对应 |
| `device_id` | string | 自动生成 | 设备唯一标识（UUID v4），自动生成无需手动填写 |
| `device_name` | string | 主机名 | 设备显示名称，会显示在其他设备的托盘菜单中 |
| `auto_sync` | bool | `true` | 启动时是否开启自动同步 |
| `auto_sync_max_bytes` | usize | `10485760` | 自动同步大小上限（字节），超限需手动上传/下载 |
| `ws_inline_max_bytes` | usize | `1048576` | WebSocket 内联传输上限（字节），文本超限会走 HTTP 通道 |
| `http_timeout_secs` | u64 | `180` | HTTP 请求超时时间（秒） |
| `autostart` | bool | `false` | 是否开机自启（写入注册表 `HKCU\Run`） |
| `hotkey_copy` | string | `Ctrl+Shift+C` | 复制并同步热键 |
| `hotkey_paste` | string | `Ctrl+Shift+V` | 同步并粘贴热键 |
| `hotkey_toggle` | string | `Ctrl+Alt+V` | 开关自动同步热键 |

---

## 项目结构

```
ClipSync/
├── clipsync-server/          # 服务端
│   ├── src/
│   │   ├── main.rs           # 入口，路由注册，中间件
│   │   ├── config.rs         # 配置（环境变量）
│   │   ├── auth.rs           # HTTP Basic Auth
│   │   ├── db.rs             # SQLite 数据访问层
│   │   ├── protocol.rs       # WebSocket 消息协议定义
│   │   ├── routes/
│   │   │   ├── health.rs     # /health 健康检查
│   │   │   ├── sync_profile.rs  # /SyncClipboard.json CRUD
│   │   │   ├── file.rs       # /file/* 文件上传下载
│   │   │   └── mod.rs        # 路由汇总
│   │   └── ws/
│   │       ├── handler.rs    # WebSocket 连接处理
│   │       ├── session.rs    # 会话管理与广播
│   │       └── mod.rs
│   └── Cargo.toml
│
├── clipsync-windows/         # Windows 客户端
│   ├── src/
│   │   ├── main.rs           # 入口，托盘图标，消息循环，热键注册
│   │   ├── config.rs         # 配置读写（TOML），自启注册表
│   │   ├── client.rs         # HTTP/WS 客户端
│   │   ├── clipboard.rs      # 剪贴板读写（文本/图片/文件 CF_HDROP）
│   │   ├── protocol.rs       # 与服务端一致的协议定义
│   │   ├── sync.rs           # 同步引擎（WS + 轮询 + 命令通道）
│   │   └── command.rs        # 同步命令枚举（SyncUpload 等）
│   └── Cargo.toml
│
├── .gitignore
├── LICENSE
└── README.md
```

---

## 构建

```bash
# 服务端
cd clipsync-server
cargo build --release

# Windows 客户端（需要 Windows 环境 + Rust MSVC toolchain）
cd clipsync-windows
cargo build --release
```

---

## 开发计划

- [x] Windows 客户端（托盘图标、热键、文件同步）
- [x] Linux 服务端（WebSocket + REST API）
- [ ] Android 客户端（纯 Kotlin）
- [ ] WebDAV 大文件传输支持
- [ ] 加密传输（TLS）

---

## 许可证

[MIT](LICENSE)
