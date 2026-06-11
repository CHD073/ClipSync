# ClipSync

> 自托管跨设备剪贴板实时同步工具。Windows ↔ Server ↔ Android，一次复制，所有设备立即可用。

## 目录

- [架构](#架构)
- [快速开始](#快速开始)
- [功能特性](#功能特性)
- [配置参考](#配置参考)
- [协议设计](#协议设计)
- [服务端](#服务端)
- [Windows 客户端](#windows-客户端)
- [Android 客户端](#android-客户端)
- [项目结构](#项目结构)
- [构建](#构建)
- [安全说明](#安全说明)
- [故障排除](#故障排除)
- [许可](#许可)

---

## 架构

```
┌──────────────┐     WebSocket      ┌──────────────┐     WebSocket      ┌──────────────┐
│  Windows     │◄──────────────────►│   Server      │◄──────────────────►│  Android     │
│  (Rust 托盘)  │    HTTPS REST      │  (Rust/Axum)  │   Shizuku 后台读   │  (Kotlin)    │
└──────────────┘                    └──────┬───────┘                    └──────────────┘
                                          │
                                 ┌────────▼────────┐
                                 │  SQLite + 文件  │
                                 │  (离线 Backlog)  │
                                 └────────────────┘
```

| 组件 | 语言 | 核心依赖 | 运行方式 |
|------|------|----------|----------|
| **Server** | Rust | Axum, tokio-tungstenite, rusqlite, rustls | 长期运行（systemd / Docker） |
| **Windows Client** | Rust | tray-icon, arboard, reqwest, windows-sys | 系统托盘图标，无窗口，开机自启 |
| **Android Client** | Kotlin | OkHttp, Jetpack Compose, Gson, Shizuku | 前台 Service + Shizuku UserService |

---

## 快速开始

```bash
# 服务端
git clone https://github.com/CHD073/ClipSync.git && cd ClipSync/clipsync-server
cargo build --release
export CLIPSYNC_TOKEN="your_secret_token"
./target/release/clipsync-server

# Windows 客户端
cd clipsync-windows && cargo build --release
# 双击 clipsync.exe，编辑 config.toml 填入 server_url + token

# Android 客户端
cd clipsync-android && ./gradlew assembleDebug
# 安装 APK → 填 Server URL → Shizuku 中授权 → Start
```

---

## 功能特性

### 同步内容

| 类型 | 传输方式 | 上限 | 说明 |
|------|----------|------|------|
| **文本** | WebSocket 内联 / HTTP 上传 | `auto_sync_max_bytes`（默认 10MB） | 小文本 WS 直传，大文本走 HTTP 文件通道 |
| **文件** | HTTP 上传 + WS 通知 | 不限制 | 可选择文件上传，通知其他设备下载 |

### 同步模式

| 模式 | 触发方式 | 说明 |
|------|----------|------|
| **Auto Sync** | 剪贴板变化自动推送 | 可独立开关，不影响手动上传/下载 |
| **Manual Upload** | 托盘菜单 / 热键 / App 按钮 | 立即上传当前剪贴板内容 |
| **Manual Download** | 托盘菜单 / 热键 / App 按钮 | 从服务端拉取最新内容并写入剪贴板 |

### Windows 客户端特性

- 系统托盘图标运行，**无窗口无控制台**
- 托盘图标颜色：🟢绿色已连接 / 🔴红色断开 / 🔵蓝色同步中
- 全局热键：
  - `Ctrl+Shift+C` — 复制并同步到所有设备
  - `Ctrl+Shift+V` — 拉取最新内容并粘贴
  - `Ctrl+Alt+V` — 切换自动同步开关
- 托盘菜单：
  - 连接状态 + 最后同步时间 + 来源设备名
  - Upload / Download — 手动同步
  - **Auto-Sync** — 开关自动同步
  - **Settings** → 编辑配置文件 / 打开配置目录
  - **Launch at Startup** — 开机自启
  - Open Log / Restart / Quit
- 单实例保护（Windows 全局互斥锁）
- 中英双语菜单（Settings → 中文/English），持久化存储
- 优雅关闭（退出时通知服务端断连）
- 流式文件下载（大文件不占满内存）

### Android 客户端特性

- Jetpack Compose 单页 UI：Shizuku 状态卡 + Server 配置 + Start/Stop + Auto-Sync 开关 + Actions + 日志面板
- 前台 Service 常驻运行，通知栏可控
- 后台剪贴板同步：通过 **Shizuku UserService** 调用系统级 `IClipboard` API
- 手动上传/下载按钮 + 文件选择上传
- 流式上传/下载（大文件不 OOM）
- Echo 防护：500ms 冷却 + hash 去重
- App 打开即自动启动服务
- 中英自动切换（随系统语言）

---

## 协议设计

### WebSocket 消息

客户端与服务端通过 WebSocket 长连接通信，消息格式为 JSON，通过 `type` 字段区分。Basic Auth 鉴权（用户名空，密码=token）。

**客户端 → 服务端：**

| 消息 | type | 说明 |
|------|------|------|
| `Auth` | `"Auth"` | `{"type":"Auth","token":"...","device_id":"...","name":"..."}` |
| `ClipSync` | `"ClipSync"` | `{"type":"ClipSync","payload":{...},"device_id":"..."}` |
| `GetLatest` | `"GetLatest"` | `{"type":"GetLatest"}` |

**服务端 → 客户端：**

| 消息 | type | 说明 |
|------|------|------|
| `AuthOk` | `"AuthOk"` | `{"type":"AuthOk","device_id":"..."}` |
| `AuthError` | `"AuthError"` | `{"type":"AuthError","reason":"..."}` |
| `ClipBroadcast` | `"ClipBroadcast"` | `{"type":"ClipBroadcast","payload":{...},"source_device_id":"...","source_device_name":"..."}` |
| `Backlog` | `"Backlog"` | `{"type":"Backlog","entries":[...]}` 离线消息列表 |
| `LatestProfile` | `"LatestProfile"` | `{"type":"LatestProfile","payload":{...},"source_device_id":"...","created_at":"..."}` |

### REST API

| 方法 | 路径 | 认证 | 说明 |
|------|------|------|------|
| `GET` / `PUT` | `/profile/latest` | Basic Auth | 获取/更新最新剪贴板 |
| `GET` / `PUT` | `/file/{name}` | Basic Auth | 二进制文件上传/下载 |
| `GET` | `/health` | 无需 | `{"service":"clipsync-server","status":"ok","version":"0.1.0"}` |
| `GET` | `/api/time` | 无需 | 服务端时间戳 |

### ProfileDto

```json
{
    "type": "Text | File",
    "hash": "SHA-256 hex string",
    "text": "文本内容（Text 类型）",
    "has_data": true,
    "data_name": "文件名（超限走文件通道时）",
    "size": 12345
}
```

---

## 服务端

### 环境要求

- Linux（任何发行版）
- Rust 工具链（1.70+）
- 开放端口（默认 8765）

### 快速部署

```bash
git clone https://github.com/CHD073/ClipSync.git
cd ClipSync/clipsync-server
cargo build --release

# 设置环境变量
export CLIPSYNC_PORT=8765
export CLIPSYNC_TOKEN="your_random_secret_token"
export CLIPSYNC_STORAGE_PATH="/opt/clipsync/data"

# 可选：HTTPS
export CLIPSYNC_TLS_CERT_PATH="/etc/letsencrypt/live/example.com/fullchain.pem"
export CLIPSYNC_TLS_KEY_PATH="/etc/letsencrypt/live/example.com/privkey.pem"

./target/release/clipsync-server
```

### systemd 服务

```ini
# /etc/systemd/system/clipsync.service
[Unit]
Description=ClipSync Server
After=network.target

[Service]
Type=simple
ExecStart=/opt/clipsync/clipsync-server
Environment=CLIPSYNC_TOKEN=your_secret_token
Environment=CLIPSYNC_STORAGE_PATH=/var/lib/clipsync
Restart=always
RestartSec=3
User=clipsync

[Install]
WantedBy=multi-user.target
```

```bash
sudo useradd -r -s /bin/false clipsync
sudo mkdir -p /var/lib/clipsync
sudo chown clipsync:clipsync /var/lib/clipsync
sudo cp target/release/clipsync-server /opt/clipsync/
sudo systemctl enable --now clipsync
```

### 数据存储

- 数据库：`{CLIPSYNC_STORAGE_PATH}/clipsync.db`（SQLite，WAL 模式）
- 文件：`{CLIPSYNC_STORAGE_PATH}/files/`
- 历史记录保留 `CLIPSYNC_MAX_HISTORY_DAYS` 天（默认 7 天）

### 数据表结构

```sql
-- 设备表
CREATE TABLE devices (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id   TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL DEFAULT '',
    last_seen   TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 剪贴板历史
CREATE TABLE clipboard_history (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    hash        TEXT NOT NULL,
    content_type TEXT NOT NULL,
    text        TEXT NOT NULL DEFAULT '',
    data_name   TEXT,
    size        INTEGER NOT NULL DEFAULT 0,
    device_id   TEXT NOT NULL,
    has_data    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
```

---

## Windows 客户端

### 环境要求

- Windows 10/11（64 位）
- Rust MSVC toolchain + Visual Studio Build Tools

### 构建

```bash
cd clipsync-windows
cargo build --release
# 输出：target/release/clipsync.exe
```

### 使用

1. 将 `clipsync.exe` 放到任意目录
2. 首次运行自动生成 `config.toml`
3. 编辑 `config.toml` 填入服务端 URL 和 Token
4. 双击运行，托盘图标出现即表示已启动

### 配置文件

`config.toml` 与 `clipsync.exe` 同目录。首次运行自动生成：

```toml
server_url = "http://192.168.1.100:8765"
token = "clipsync"
device_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"  # 自动生成 UUID
device_name = "MyPC"  # 默认取主机名
auto_sync = true
auto_sync_max_bytes = 10485760
ws_inline_max_bytes = 1048576
http_timeout_secs = 180
autostart = false
hotkey_copy = "Ctrl+Shift+C"
hotkey_paste = "Ctrl+Shift+V"
hotkey_toggle = "Ctrl+Alt+V"
```

### 托盘图标说明

| 颜色 | 状态 |
|------|------|
| 🟢 绿色 | 已连接服务器 |
| 🔴 红色 | 断开连接 |
| 🔵 蓝色闪烁 | 正在同步（上传或下载） |

### 热键自定义

热键格式：`修饰键+修饰键+键名`

| 修饰键 | 写法 |
|--------|------|
| Ctrl | `Ctrl` |
| Shift | `Shift` |
| Alt | `Alt` |
| Win | `Win` |

键名：单个字母 `A`-`Z` 或数字 `0`-`9`。

---

## Android 客户端

### 环境要求

- Android 9.0+（minSdk 28）
- JDK 17
- Android SDK（compileSdk 35）

### Shizuku 依赖

Android 客户端通过 **Shizuku UserService** 实现后台剪贴板读取。Shizuku 是一个无需 Root 即可调用系统 API 的框架。

**安装与配置：**

1. 安装 [Shizuku App](https://shizuku.rikka.app/)（moe.shizuku.privileged.api）
2. 通过 ADB 或无线调试启动 Shizuku Server：
   ```bash
   # ADB 方式
   adb shell sh /storage/emulated/0/Android/data/moe.shizuku.privileged.api/start.sh
   
   # 或直接调用 native 库（Android 13+）
   adb shell /data/app/~~XXXX==/moe.shizuku.privileged.api-XXXX==/lib/arm64/libshizuku.so
   ```
3. 打开 ClipSync App → 在 Shizuku 中授权 ClipSync
4. Shizuku 卡片变绿「Ready」即就绪

> **注意：** 设备重启后需重新执行步骤 2 启动 Shizuku Server。

### 构建

```bash
cd clipsync-android
./gradlew assembleDebug
# 输出：app/build/outputs/apk/debug/app-debug.apk
```

### UI 指南

```
┌─────────────────────────────────┐
│  ClipSync                       │
│                                 │
│  ┌─────────────────────────┐    │
│  │ ✅ Shizuku Ready         │    │  ← 绿/橙/红三态
│  └─────────────────────────┘    │
│                                 │
│  Server ▼                       │  ← 折叠的 Server 配置
│  URL: [___________________]     │
│  Token: [_________________]     │
│  [Save]                         │
│  Device: [________________]     │  ← 设备显示名
│                                 │
│  🟢 Connected                   │
│                                 │
│  [ ▶ Start ]                    │  ← 控制前台 Service
│                                 │
│  Auto Sync  [====🔘====]        │  ← 独立开关
│                                 │
│  Actions                        │
│  [Upload] [Download]            │
│  [Upload File]                  │
│                                 │
│  Last Sync                      │
│  12:34:56  From: MECHREVO-14X   │
│                                 │
│  Log                            │
│  ┌─────────────────────────┐    │
│  │ ...                      │    │
│  └─────────────────────────┘    │
└─────────────────────────────────┘
```

### 后台同步原理

```
用户复制文字
  ↓
前台 Service 保持进程在 foreground 状态
  ↓
cm.primaryClip 读剪贴板（前台可直接读）
  ↓
若 cm 返回旧缓存 → ShizukuShell.getText()
  ↓
Shizuku UserService (UID 2000/shell)
  ↓
反射调用 IClipboard.getPrimaryClip("com.android.shell")
  ↓
返回最新 ClipData → 提取文本 → WS 推送
```

### 首次使用流程

1. 打开 ClipSync
2. 展开 Server 配置 → 填入 URL + Token → Save
3. 确认 Shizuku 卡片为绿色（如不是，在 Shizuku App 中重新授权）
4. 点击 ▶ Start 启动服务
5. 确认 Auto Sync 开关开启
6. 切到其他 App → 复制文字 → 所有设备自动同步

---

## 配置参考

### 服务端（环境变量）

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `CLIPSYNC_PORT` | `8765` | 监听端口 |
| `CLIPSYNC_TOKEN` | `clipsync` | ⚠️ 生产环境必须修改 |
| `CLIPSYNC_STORAGE_PATH` | `./data` | 数据库 + 上传文件目录 |
| `CLIPSYNC_MAX_HISTORY_DAYS` | `7` | 历史记录保留天数 |
| `CLIPSYNC_BIND_ADDR` | `0.0.0.0` | 监听地址 |
| `CLIPSYNC_TLS_CERT_PATH` | — | TLS 证书路径（同时设置两个才启用 HTTPS） |
| `CLIPSYNC_TLS_KEY_PATH` | — | TLS 私钥路径 |

### Windows 客户端（config.toml）

| 参数 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `server_url` | string | — | 服务端地址 |
| `token` | string | — | 认证令牌 |
| `device_id` | string | 自动生成 | UUID v4，自动生成无需修改 |
| `device_name` | string | 主机名 | 显示名称 |
| `auto_sync` | bool | `true` | 启动时是否开启自动同步 |
| `auto_sync_max_bytes` | int | `10485760` | 自动同步大小上限（字节） |
| `ws_inline_max_bytes` | int | `1048576` | WS 内联传输上限 |
| `http_timeout_secs` | int | `180` | HTTP 请求超时（秒） |
| `autostart` | bool | `false` | 开机自启（注册表 HKCU\Run） |
| `language` | string | `"en"` | 语言：`"en"` / `"zh"` |
| `hotkey_copy` | string | `Ctrl+Shift+C` | 复制并同步热键 |
| `hotkey_paste` | string | `Ctrl+Shift+V` | 同步并粘贴热键 |
| `hotkey_toggle` | string | `Ctrl+Alt+V` | 切换自动同步热键 |

### Android 客户端（SharedPreferences）

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `server_url` | — | 服务端地址 |
| `token` | `clipsync` | 认证令牌 |
| `device_name` | 厂商+型号 | 设备显示名 |
| `device_id` | 自动生成 | UUID v4 |
| `auto_sync` | `true` | 后台轮询开关 |
| `auto_sync_max_bytes` | `10485760` | 自动同步大小上限 |

---

## 项目结构

```
ClipSync/
├── clipsync-server/              # Rust 服务端
│   ├── src/
│   │   ├── main.rs               # 入口，路由注册，TLS 配置
│   │   ├── config.rs             # 环境变量配置
│   │   ├── auth.rs               # HTTP Basic Auth
│   │   ├── db.rs                 # SQLite 数据访问（WAL 模式）
│   │   ├── protocol.rs           # WebSocket 消息协议
│   │   ├── routes/
│   │   │   ├── health.rs         # /health 健康检查
│   │   │   ├── sync_profile.rs   # /profile/latest CRUD
│   │   │   ├── file.rs           # /file/* 上传下载（路径穿越防护）
│   │   │   └── mod.rs
│   │   └── ws/
│   │       ├── handler.rs        # WebSocket 连接处理（孤儿任务防护）
│   │       ├── session.rs        # 广播频道
│   │       └── mod.rs
│   └── Cargo.toml
│
├── clipsync-windows/             # Rust Windows 客户端
│   ├── src/
│   │   ├── main.rs               # 托盘图标 + 热键 + 消息泵 + 优雅关闭
│   │   ├── config.rs             # TOML 配置 + 注册表自启
│   │   ├── client.rs             # HTTP/WS 客户端（流式文件下载）
│   │   ├── clipboard.rs          # 剪贴板读写（文本/图片/CF_HDROP）
│   │   ├── protocol.rs           # DTO + WS 消息解析
│   │   ├── sync.rs               # 同步引擎（WS + 轮询 + 退出信号）
│   │   └── command.rs            # 命令枚举 + 状态结构
│   └── Cargo.toml
│
├── clipsync-android/             # Kotlin Android 客户端
│   ├── app/src/main/
│   │   ├── aidl/com/clipsync/app/
│   │   │   └── IClipSyncService.aidl      # AIDL 接口定义
│   │   ├── java/com/clipsync/app/
│   │   │   ├── MainActivity.kt            # Compose UI
│   │   │   ├── SyncManager.kt             # 同步逻辑 + Echo 防护
│   │   │   ├── WsClient.kt                # OkHttp WebSocket
│   │   │   ├── HttpApi.kt                 # HTTP 流式上传/下载
│   │   │   ├── Protocol.kt                # Gson DTO + WS 解析
│   │   │   ├── Config.kt                  # SharedPreferences
│   │   │   ├── ClipSyncApp.kt             # Application + Shizuku 初始化
│   │   │   ├── SyncService.kt             # 前台 Service
│   │   │   ├── ClipboardShell.kt          # Shizuku 状态封装
│   │   │   ├── ShizukuCompat.kt           # Kotlin 反射访问 Shizuku
│   │   │   ├── ShizukuShell.kt            # 绑定 UserService + 读剪贴板
│   │   │   └── ClipSyncUserService.kt     # Shizuku UserService（shell UID）
│   │   ├── java/rikka/shizuku/
│   │   │   └── ShizukuProvider.kt         # 标准 ContentProvider（binder 投递）
│   │   ├── java/moe/shizuku/api/
│   │   │   └── BinderContainer.java       # Parcelable 桥接
│   │   ├── res/
│   │   │   ├── values/strings.xml
│   │   │   ├── values/themes.xml
│   │   │   └── xml/file_paths.xml
│   │   └── AndroidManifest.xml
│   ├── app/build.gradle.kts
│   ├── build.gradle.kts
│   └── settings.gradle.kts
│
├── .gitignore
├── LICENSE
└── README.md
```

---

## 构建

```bash
# 服务端
cd clipsync-server && cargo build --release

# Windows 客户端（需要 Windows + MSVC toolchain）
cd clipsync-windows && cargo build --release

# Android 客户端（需要 Android SDK + JDK 17）
cd clipsync-android && ./gradlew assembleDebug
```

---

## 安全说明

| 风险 | 等级 | 建议 |
|------|------|------|
| 默认 Token | 🔴高 | 生产环境必须设置 `CLIPSYNC_TOKEN` 为随机长字符串 |
| 明文传输 | 🔴高 | 配置 TLS 证书：`CLIPSYNC_TLS_CERT_PATH` + `CLIPSYNC_TLS_KEY_PATH` |
| 请求体无限 | 🟡中 | 服务端无请求大小限制，建议用 nginx 前置限制 |
| 速率限制 | 🟡中 | 无内置限流，建议 nginx / cloudflare 防护 |
| DB 无加密 | 🟡中 | SQLite 文件明文，确保存储目录仅服务运行用户可读 |
| 剪贴板内容敏感 | 🟡中 | HTTP 下所有内容明文传输，务必启用 TLS |

---

## 故障排除

### 服务端无法启动

```bash
# 检查端口占用
ss -tlnp | grep 8765

# 检查存储目录权限
ls -la /opt/clipsync/data/

# 查看日志
journalctl -u clipsync -f
```

### Windows 客户端无托盘图标

- 确认没有其他 ClipSync 实例在运行（单实例保护）
- 检查 `clipsync.log` 同目录下的日志文件
- 部分 RDP/远程桌面下托盘图标可能不显示，功能正常

### Android 后台同步不工作

1. 确认 Shizuku 卡片为绿色「Ready」
2. 检查 Server URL 和 Token 配置
3. 确认 Auto Sync 开关开启
4. 确认手机和服务器在同一网络（可互相 ping 通）
5. 重启 Shizuku Server：`adb shell /path/to/libshizuku.so`

### Android Shizuku 卡片显示红色/橙色

- **红色「Not running」**：Shizuku Server 未启动，执行 ADB 启动命令
- **橙色「Not authorized」**：在 Shizuku App 中找到 ClipSync，关掉授权再重新打开

### PC 收不到手机内容

1. 确认手机和 PC 连接同一个 Server
2. 手机上确认 Auto Sync 开启且 WS 已连接（显示 🟢 Connected）
3. 在手机前台打开 ClipSync，复制文字测试是否能同步（前台同步验证链路）
4. 若前台可同步但后台不行：重启 Shizuku Server，重新授权

---

## 许可

[MIT](LICENSE)
