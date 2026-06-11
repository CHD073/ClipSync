# ClipSync

> 自托管跨设备剪贴板同步工具。Windows ↔ Server ↔ Android，一次复制，所有设备立即可用。

## 架构

```
┌──────────────┐     WebSocket     ┌──────────────┐     WebSocket     ┌──────────────┐
│  Windows     │◄─────────────────►│   Server      │◄─────────────────►│  Android      │
│  (Rust 托盘)  │    HTTPS REST    │   (Rust/Axum) │   Shizuku 读写    │  (Kotlin)     │
└──────────────┘                   └──────┬───────┘                   └──────────────┘
                                          │
                                 ┌────────▼────────┐
                                 │  SQLite + files  │
                                 │  (离线 Backlog)   │
                                 └─────────────────┘
```

| 组件 | 语言 | 核心依赖 | 运行方式 |
|------|------|----------|----------|
| **Server** | Rust | Axum, tokio-tungstenite, rusqlite | 长期运行（systemd / 裸机） |
| **Windows Client** | Rust | tray-icon, arboard, reqwest | 托盘图标，无窗口 |
| **Android Client** | Kotlin | OkHttp, Jetpack Compose, Shizuku API | 前台 Service |

---

## 功能

### 同步内容

| 类型 | 传输方式 | 上限 |
|------|----------|------|
| 文本 | WebSocket 内联 | `ws_inline_max_bytes`（默认 1MB） |
| 文件 | HTTP 上传 + WS 通知 | `auto_sync_max_bytes`（默认 10MB），超限需手动 |

### 同步模式

| 模式 | 触发方式 | 说明 |
|------|----------|------|
| **Auto Sync** | 剪贴板变化自动推送 | 可独立开关 |
| **Upload** | 手动点击 / 热键 | 上传当前剪贴板内容 |
| **Download** | 手动点击 / 热键 | 从服务端拉取最新内容 |

### Windows 客户端

- 托盘图标（绿色=已连接，红色=断开，蓝色闪烁=正在同步）
- 全局热键：`Ctrl+Shift+C`（复制并同步）、`Ctrl+Shift+V`（同步并粘贴）、`Ctrl+Alt+V`（切换自动同步）
- 托盘菜单：Upload / Download / Auto-Sync 开关 / Settings / Open Log / Restart / Quit
- 开机自启（写入注册表 HKCU\Run）
- 单实例保护（互斥锁）
- 支持文件粘贴（CF_HDROP）

### Android 客户端

**后台剪贴板读取实现方案：** 通过 Shizuku 框架调用系统级 shell 命令读取剪贴板，绕过 Android 10+ 的后台限制。

| 方案 | 状态 | 说明 |
|------|------|------|
| Shizuku | ✅ 可用 | 后台读写剪贴板，覆盖所有 App |
| AccessibilityService | ❌ 已移除 | 被 Shizuku 取代 |

#### 使用前提

1. 安装 [Shizuku App](https://shizuku.rikka.app/)（moe.shizuku.privileged.api）
2. 在 Shizuku 中授权 ClipSync

**首次使用流程：**
1. 打开 ClipSync，确认 Shizuku 卡片状态
2. 如显示红色「Not running」，在 Shizuku 中重新授权（关掉再打开）
3. 执行上方 ADB 命令
4. 回到 ClipSync，Shizuku 卡片应变为绿色「Ready」
5. 设置 Server URL + Token，点 **Start**
6. **Auto Sync** 开关控制后台轮询

---

## 协议

### WebSocket 消息

**客户端 → 服务端：**

| 消息 | 用途 |
|------|------|
| `Auth` | 认证（token + device_id + 设备名） |
| `ClipSync` | 推送新内容，附 `ProfileDto` |
| `GetLatest` | 请求最新内容 |

**服务端 → 客户端：**

| 消息 | 用途 |
|------|------|
| `AuthOk` / `AuthError` | 认证结果 |
| `ClipBroadcast` | 广播内容到所有在线设备 |
| `Backlog` | 离线遗漏内容 |
| `LatestProfile` | 最新条目（含来源设备） |

### REST API

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| `GET` / `PUT` | `/profile/latest` | Basic Auth | 获取/更新最新剪贴板 |
| `GET` / `PUT` | `/file/{name}` | Basic Auth | 二进制文件上传下载 |
| `GET` | `/health` | 无需 | 健康检查 |
| `GET` | `/api/time` | 无需 | 服务端时间戳 |

**认证方式：** HTTP Basic Auth，用户名为空，密码为 `token`。

### ProfileDto

```json
{
    "type": "Text | File",
    "hash": "SHA-256 hex",
    "text": "文本内容",
    "has_data": true,
    "data_name": "文件名",
    "size": 12345
}
```

---

## 快速开始

### 服务端

```bash
git clone https://github.com/CHD073/ClipSync.git
cd ClipSync/clipsync-server
cargo build --release

# 配置
export CLIPSYNC_PORT=8765
export CLIPSYNC_TOKEN="your_secret_token"
export CLIPSYNC_STORAGE_PATH="/opt/clipsync/data"

# 可选：TLS
export CLIPSYNC_TLS_CERT_PATH="/etc/letsencrypt/live/.../fullchain.pem"
export CLIPSYNC_TLS_KEY_PATH="/etc/letsencrypt/live/.../privkey.pem"

./target/release/clipsync-server
```

#### systemd 服务

```ini
[Unit]
Description=ClipSync Server
After=network.target

[Service]
Type=simple
ExecStart=/opt/clipsync/clipsync-server
Environment=CLIPSYNC_TOKEN=my_token
Environment=CLIPSYNC_STORAGE_PATH=/var/lib/clipsync
Restart=always
RestartSec=3
User=clipsync

[Install]
WantedBy=multi-user.target
```

### Windows 客户端

```bash
cd clipsync-windows
cargo build --release
```

运行 `clipsync-windows/target/release/clipsync.exe`，托盘图标出现即运行中。同目录下会自动生成/读取 `config.toml`。

### Android 客户端

```bash
cd clipsync-android
./gradlew assembleDebug
# APK 位置: app/build/outputs/apk/debug/app-debug.apk
```

安装 APK 后按上方「首次使用流程」配置。

---

## 配置

### 服务端（环境变量）

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `CLIPSYNC_PORT` | `8765` | 监听端口 |
| `CLIPSYNC_TOKEN` | `clipsync` | ⚠️ 生产环境必须修改 |
| `CLIPSYNC_STORAGE_PATH` | `./data` | 数据库 + 文件存储路径 |
| `CLIPSYNC_MAX_HISTORY_DAYS` | `7` | 历史保留天数 |
| `CLIPSYNC_TLS_CERT_PATH` | — | TLS 证书路径 |
| `CLIPSYNC_TLS_KEY_PATH` | — | TLS 私钥路径 |
| `CLIPSYNC_BIND_ADDR` | `0.0.0.0` | 监听地址 |

### Windows 客户端（config.toml）

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `server_url` | string | — | 服务端 URL |
| `token` | string | — | 认证令牌 |
| `device_id` | string | 自动生成 | UUID v4 |
| `device_name` | string | 主机名 | 显示名称 |
| `auto_sync` | bool | `true` | 自动同步 |
| `auto_sync_max_bytes` | int | `10485760` | 自动同步大小上限 |
| `ws_inline_max_bytes` | int | `1048576` | WS 内联上限 |
| `http_timeout_secs` | int | `180` | HTTP 超时 |
| `autostart` | bool | `false` | 开机自启 |
| `hotkey_copy` | string | `Ctrl+Shift+C` | 复制并同步 |
| `hotkey_paste` | string | `Ctrl+Shift+V` | 同步并粘贴 |
| `hotkey_toggle` | string | `Ctrl+Alt+V` | 切换自动同步 |

### Android 客户端（内置 SharedPreferences）

| 参数 | 默认值 | 说明 |
|------|--------|------|
| Server URL | — | 服务端地址 |
| Token | `clipsync` | 认证令牌 |
| Device Name | 厂商+型号 | 设备显示名称 |
| Auto Sync | `true` | 后台轮询开关 |

---

## 项目结构

```
ClipSync/
├── clipsync-server/          # Rust 服务端
│   ├── src/
│   │   ├── main.rs           # 入口 + TLS + 中间件
│   │   ├── config.rs         # 环境变量配置
│   │   ├── auth.rs           # Basic Auth
│   │   ├── db.rs             # SQLite（WAL 模式）
│   │   ├── protocol.rs       # 消息协议
│   │   ├── routes/
│   │   │   ├── health.rs     # 健康检查
│   │   │   ├── sync_profile.rs
│   │   │   ├── file.rs       # 文件（路径穿越防护）
│   │   │   └── mod.rs
│   │   └── ws/
│   │       ├── handler.rs    # WS 连接（孤儿任务防护）
│   │       ├── session.rs    # 广播频道
│   │       └── mod.rs
│   └── Cargo.toml
│
├── clipsync-windows/         # Rust Windows 客户端
│   ├── src/
│   │   ├── main.rs           # 托盘 + 热键 + 消息泵 + 优雅关闭
│   │   ├── config.rs         # TOML 配置 + 注册表自启
│   │   ├── client.rs         # HTTP/WS 客户端（流式文件下载）
│   │   ├── clipboard.rs      # 剪贴板（文本/图片/CF_HDROP）
│   │   ├── protocol.rs       # 协议 DTO
│   │   ├── sync.rs           # 同步引擎（WS + 轮询 + 退出信号）
│   │   └── command.rs        # 命令枚举
│   └── Cargo.toml
│
├── clipsync-android/         # Kotlin Android 客户端
│   ├── app/src/main/java/com/clipsync/app/
│   │   ├── MainActivity.kt       # Compose UI
│   │   ├── SyncManager.kt        # 同步逻辑
│   │   ├── WsClient.kt           # OkHttp WebSocket
│   │   ├── HttpApi.kt            # HTTP 流式上传/下载
│   │   ├── Protocol.kt           # Gson DTO + WS 解析
│   │   ├── Config.kt             # SharedPreferences
│   │   ├── ClipSyncApp.kt        # Application + Shizuku listener
│   │   ├── SyncService.kt        # 前台 Service
│   │   ├── ClipboardShell.kt     # Shizuku 剪贴板封装
│   │   ├── ShizukuCompat.kt      # 反射调用 newProcess
│   │   ├── ShizukuApiProvider.kt # ContentProvider 接收 binder
│   │   └── moe/shizuku/api/
│   │       └── BinderContainer.java  # Parcelable 桥接
│   ├── app/src/main/res/
│   │   ├── values/strings.xml
│   │   ├── values/themes.xml
│   │   └── xml/file_paths.xml
│   ├── app/build.gradle.kts
│   ├── build.gradle.kts
│   └── settings.gradle.kts
│
├── .gitignore
├── LICENSE
└── README.md
```

---

## 安全说明

| 风险 | 建议 |
|------|------|
| 默认 token | 生产环境设置 `CLIPSYNC_TOKEN` 为随机字符串 |
| 明文传输 | 配置 `CLIPSYNC_TLS_CERT_PATH` + `CLIPSYNC_TLS_KEY_PATH` 启用 TLS |
| 请求体无限 | 服务端未限制请求体大小，建议用反向代理（nginx）限制 |
| 速率限制 | 服务端无内置限流，建议前面加 nginx / cloudflare |
| DB 无加密 | SQLite 文件无加密，确保存储目录仅对运行用户可读 |

---

## 构建

```bash
# 服务端
cd clipsync-server && cargo build --release

# Windows 客户端（需要 Windows + MSVC toolchain）
cd clipsync-windows && cargo build --release

# Android（需要 Android SDK + JDK 17）
cd clipsync-android
./gradlew assembleDebug
```

---

## 许可证

[MIT](LICENSE)
