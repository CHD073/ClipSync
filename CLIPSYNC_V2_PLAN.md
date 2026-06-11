# ClipSync — 架构与路线图

## 需求确认清单

| 项目 | 结论 |
|------|------|
| 同步类型 | 文本 + 图片 + 文件 |
| Windows | 托盘常驻 + 全局热键 |
| Android | 前台自动 / 后台手动（混合） |
| 历史 | 不需要 |
| 后端模式 | WebDAV + 自建服务端 都要 |
| 协议 | 兼容 SyncClipboard API |
| Android 技术 | 纯 Kotlin（不共用 Rust） |
| 项目名 | ClipSync |

---

## 一、系统架构

```
                    ┌──────────────────────┐
                    │   ClipSync Server    │
                    │   (Rust / Axum)      │
                    │                      │
                    │  REST: /SyncClipboard│
                    │        /file/{name}  │
                    │  WS:   /ws 实时广播  │
                    │  DB:   SQLite        │
                    └──────┬───────────────┘
                           │ WS + HTTP
              ┌────────────┴────────────┐
              ▼                         ▼
┌──────────────────────┐   ┌──────────────────────┐
│  Windows 客户端      │   │  Android 客户端       │
│  (Rust)              │   │  (Kotlin)            │
│                      │   │                      │
│  ├─ 剪贴板监控       │   │  ├─ 前台服务保活     │
│  │  AddClipboard     │   │  ├─ 自动/手动切换    │
│  │  FormatListener   │   │  ├─ 通知栏控制       │
│  ├─ 文本/图片/文件   │   │  ├─ 文本/图片/文件   │
│  ├─ 托盘 + 热键     │   │  └─ 设置页(Compose) │
│  └─ 双模式引擎      │   │                      │
│                      │   │                      │
│  也可直连 ───────────┤   ├── 也可直连           │
│  WebDAV (Nextcloud)  │   │  WebDAV              │
└──────────────────────┘   └──────────────────────┘
```

**两端独立实现，通过同一套协议互通**。不需要在 Android 上跑 Rust。

---

## 二、通信方式

### 模式 A：ClipSync Server（自建）

```
写入流程：
  Windows/A 复制 → 监听触发 → WS 发送 → Server 广播
                                         → Android 收到广播 → 写剪贴板
  (大文件走 REST PUT/GET，WS 只传 ProfileDto)

拉取流程（后台/断连后）：
  Android 切回前台 → HTTP GET /SyncClipboard.json
                    → hash 不同 → GET /file/{name} 下载 → 写剪贴板
```

### 模式 B：WebDAV 直连

```
所有设备定时轮询同一 WebDAV 目录：
  1. GET /SyncClipboard.json（带 If-Modified-Since）
  2. hash 不同 → GET /file/{name} 下载
  3. 本机有更新 → PUT /SyncClipboard.json
               → PUT /file/{name}
```

---

## 三、组件设计

### 3.1 ClipSync Server（Rust）

| 功能 | 说明 |
|------|------|
| REST API | `GET/PUT /SyncClipboard.json` `GET/PUT /file/{name}` |
| WebSocket | `/ws` 连接管理、广播、离线 Backlog |
| SQLite | 设备注册、离线消息队列 |
| Auth | Basic Auth（兼容 SyncClipboard） |
| Health | `GET /health` `GET /api/time` |

依赖：axum + tokio + rusqlite + serde_json + sha2 + tower-http

### 3.2 Windows 客户端（Rust）

| 模块 | 说明 |
|------|------|
| 剪贴板监听 | clipboard-win `AddClipboardFormatListener` |
| 文本读/写 | CF_UNICODETEXT / arboard |
| 图片读/写 | CF_DIBV5 → image crate PNG 编解码 |
| 文件读/写 | CF_HDROP 枚举 → 读取内容 / 重建临时文件 |
| 去重 | SHA256 + device_id |
| WebDAV 引擎 | reqwest PROPFIND/PUT/GET 轮询 |
| WS 引擎 | tokio-tungstenite 连接 Server |
| 系统托盘 | tray-icon 三态图标 + 右键菜单 |
| 全局热键 | rdev Ctrl+Shift+F1/F2 |
| 配置 | JSON，`%APPDATA%/clipsync/config.json` |

### 3.3 Android 客户端（Kotlin）

| 模块 | 说明 |
|------|------|
| 前台服务 | `ForegroundService` + 常驻通知 |
| 剪贴板监听 | `ClipboardManager.OnPrimaryClipChangedListener` |
| 文本读/写 | `ClipData.newPlainText()` / `getPrimaryClip()` |
| 图片读/写 | 存文件 → `FileProvider` URI → ClipData |
| 文件读/写 | `ContentResolver` 流读写 |
| 自动模式 | App 可见时自动同步 |
| 手动模式 | App 后台时通知栏"拉取"按钮 |
| 通知栏 | 状态显示 + 快捷操作 |
| 设置界面 | Jetpack Compose：服务器地址 / 模式切换 / 令牌配置 |
| HTTP 客户端 | OkHttp / Ktor Client |
| WS 客户端 | OkHttp WebSocket |
| WebDAV | OkHttp 手动实现 PROPFIND/PUT/GET |

---

## 四、路线图

### 里程碑 1 — 服务端 + 协议（第 1 周）

```
Day 1-2   Axum 骨架，REST 路由 /SyncClipboard.json /file/{name}
Day 3-4   WebSocket 连接管理 + 广播 + Backlog
Day 5     SQLite 设备注册 + Auth + 健康检查
         → 可用 curl/Postman 验证完整协议
```

### 里程碑 2 — Windows 客户端 MVP（第 2-3 周）

```
Day 1-3   剪贴板监控 + 文本/图片/文件读写 + SHA256 去重
Day 4-6   WebDAV 同步引擎（轮询 + 上传/下载）
Day 7-9   WS 引擎（连 Server、收发、广播过滤）
Day 10-12 系统托盘 + 全局热键 + 配置持久化
         → Windows 端可用，两台 PC 互通
```

### 里程碑 3 — Android 客户端（第 3-5 周）

```
Day 1-3   前台服务 + 剪贴板监听 + 通知栏
Day 4-6   WebDAV 模式实现（轮询 + 上传/下载）
Day 7-9   WS 模式实现（OkHttp WS）
Day 10-12 混合模式逻辑（前台自动/后台手动切换）
Day 13-15 设置界面（Compose）+ 端到端联调
         → 全链路贯通
```

### 并行策略

```
Week 1    ─── Server ───
Week 2    ─── Windows 剪贴板 ───
Week 3    ─── Windows 引擎+托盘 ───     Android 起跑
Week 4    ─── 联调 ───     ─── Android ───
Week 5    ─── 集成测试 + 修bug ───
```

**总工期：约 5 周**

---

## 五、核心文件清单

### Server（Rust）

```
server/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── config.rs
    ├── auth.rs
    ├── db.rs
    ├── routes/
    │   ├── mod.rs
    │   ├── sync_profile.rs   # GET/PUT /SyncClipboard.json
    │   ├── file.rs           # GET/PUT /file/{name}
    │   └── health.rs
    └── ws/
        ├── mod.rs
        ├── handler.rs        # WS 升级 + 消息分发
        └── session.rs        # 连接管理 + 广播
```

### Windows Client（Rust）

```
client-win/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── config.rs
    ├── clipboard/
    │   ├── mod.rs
    │   ├── monitor.rs
    │   ├── reader.rs
    │   └── writer.rs
    ├── sync/
    │   ├── mod.rs
    │   ├── webdav.rs
    │   └── clipsync.rs
    ├── tray.rs
    └── hotkey.rs
```

### Android Client（Kotlin）

```
client-android/
└── app/src/main/java/com/clipsync/
    ├── MainActivity.kt
    ├── ClipboardService.kt         # 前台服务
    ├── sync/
    │   ├── SyncManager.kt          # 自动/手动切换
    │   ├── WebdavSyncEngine.kt     # WebDAV 模式
    │   └── ClipSyncEngine.kt       # WS 模式
    ├── clipboard/
    │   ├── ClipboardMonitor.kt     # 剪贴板监听
    │   ├── ClipboardReader.kt      # 读取
    │   └── ClipboardWriter.kt      # 写入
    ├── ui/
    │   ├── SettingsScreen.kt       # 设置页 (Compose)
    │   └── MainScreen.kt           # 状态页
    └── notification/
        └── SyncNotification.kt     # 通知栏
```
