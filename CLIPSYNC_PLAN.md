# ClipSync — 跨平台剪贴板同步工具 项目策划书

## 1. 项目概述

### 1.1 项目定位

跨平台（Windows 首期→移动端）剪贴板实时同步工具，支持自建服务器实时推送和标准 WebDAV 服务器两种后端模式，兼容所有遵循 SyncClipboard REST API 的第三方客户端。

### 1.2 核心目标

- **Windows 桌面端**剪贴板自动同步（文本/图片/文件）
- **双模式同步引擎**：ClipSync 实时推送 + WebDAV 标准轮询
- **单文件部署**，零运行时依赖
- **API 兼容** SyncClipboard，第三方客户端可直接使用

### 1.3 项目口号

> 剪贴板，在你所有设备上。

---

## 2. 技术方案

### 2.1 技术栈

| 层 | 选型 | 版本 | 说明 |
|---|---|---|---|
| 语言 | Rust | edition 2024 | 单文件编译，内存安全，无 GC |
| 异步运行时 | tokio | 1.x | 全异步，共享同一运行时 |
| HTTP 服务端 | axum | 0.8 | 内置 WebSocket + Tower 中间件 |
| 数据库 | rusqlite (bundled) | 0.32 | SQLite，零外部依赖 |
| 剪贴板监控 | clipboard-win (monitor) | 5.4 | Win32 `AddClipboardFormatListener` |
| 剪贴板读写 | clipboard-win + arboard | - | 互补：文件列表 + 跨平台抽象 |
| 图片处理 | image | 0.25 | DIB↔PNG 转换 |
| 系统托盘 | tray-icon | - | 原生托盘，无 WebView |
| 序列化 | serde + serde_json | 1.x | 标准方案 |
| 日志 | tracing | 0.1 | 结构化日志，支持文件输出 |

### 2.2 系统架构

```
┌──────────────────────────────────────────────────────┐
│                    cs-server                          │
│  (可选) 自建实时服务端                                 │
│                                                      │
│  REST API         WebSocket /ws       WebDAV 兼容     │
│  /SyncClipboard   广播推送            PROPFIND/MKCOL  │
│  .json            (SignalR替代)                       │
│  /file/{name}                                        │
└──────────┬───────────────────────────────────────────┘
           │ WS + HTTP
           │
┌──────────▼───────────────────────────────────────────┐
│                    cs-client / cs-tray                 │
│                                                      │
│  ┌────────────── SYNC ENGINE ────────────────────┐   │
│  │ Mode 1: ClipSync (WS实时推送 + REST文件传输)   │   │
│  │ Mode 2: WebDAV (HTTP轮询，连Nextcloud/AList等) │   │
│  └────────────────────────────────────────────────┘   │
│                                                      │
│  ┌────────────── CLIPBOARD ──────────────────────┐   │
│  │ 监控: AddClipboardFormatListener (Vista+)     │   │
│  │ 读取: Text / Image(CF_DIBV5→PNG) / File(HDROP)│   │
│  │ 写入: 对应格式                                 │   │
│  │ 去重: SHA256 + seq_num                        │   │
│  └────────────────────────────────────────────────┘   │
│                                                      │
│  ┌────────────── SYSTEM TRAY ────────────────────┐   │
│  │ 状态图标(连接/断开/暂停) + 右键菜单            │   │
│  └────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────┘
```

### 2.3 通信协议

**ProfileDto**（与 SyncClipboard 兼容）：

```json
{
  "type": "Text | Image | File | Group",
  "hash": "sha256",
  "text": "文本内容或预览",
  "hasData": true,
  "dataName": "文件名（可选）",
  "size": 12345
}
```

**WebSocket 消息（ClipSync 模式专用）：**

```json
// 客户端→服务端
{ "type": "Auth", "token": "base64(username:password)" }

{ "type": "ClipSync", "payload": { ClipPayload } }

// 服务端→客户端
{ "type": "AuthOk", "device_id": "uuid" }

{ "type": "ClipBroadcast",
  "payload": { ClipPayload },
  "source_device_id": "..." }

{ "type": "Backlog", "entries": [...] }
```

---

## 3. 功能规格

### 3.1 MVP 功能集

| 功能 | 说明 | 优先级 |
|------|------|--------|
| 文本同步 | 纯文本跨设备复制 | P0 |
| 图片同步 | 任意格式图片→PNG 中间格式传输 | P0 |
| 文件同步 | CF_HDROP 文件列表读取/写入 | P0 |
| WebDAV 模式 | 连第三方 WebDAV 服务（Nextcloud/AList） | P0 |
| 剪贴板监听 | AddClipboardFormatListener 事件驱动 | P0 |
| 回音环防护 | SHA256 + 来源设备 ID 双重过滤 | P0 |
| 系统托盘 | 状态图标 + 右键菜单 | P0 |
| 配置文件 | JSON 文件，手动编辑 | P0 |
| 断连重试 | 指数退避重连 | P0 |
| 开机自启 | 注册表 Run 键 | P1 |

### 3.2 后续扩展

| 功能 | 说明 | 计划 |
|------|------|------|
| ClipSync 服务端 | 自建实时推送服务端 | Phase 4 |
| ClipSync WS 模式 | 客户端连自建服务端 | Phase 5 |
| 剪贴板历史 | 本地 SQLite 存储 + 浏览 | 后续 |
| 历史服务端同步 | 对标 SyncClipboard 完整历史 | 后续 |
| macOS/Linux 客户端 | arboard 交叉编译 | 后续 |
| Tauri Mobile | 安卓/iOS 客户端 | 后续 |
| 端到端加密 | XChaCha20-Poly1305 + Argon2id | 后续 |
| 图片缩略图 | 大图传输前缩略 | 后续 |

---

## 4. 实施计划

### 4.1 开发路线图

```
Phase 0 ─── 项目骨架 + 类型定义 (1天)
  │
  ▼
Phase 1 ─── Windows 剪贴板操作 (5天)
  │  └─ 此时可读/写剪贴板，打印日志
  ▼
Phase 2 ─── WebDAV 同步引擎 (4天)
  │  └─ ★ 首个可用版本！两台设备通过 WebDAV 互通
  ▼
Phase 3 ─── 系统托盘 + 后台常驻 (2天)
  │  └─ ★ 可用产品，后台运行
  ▼
Phase 4 ─── ClipSync 服务端 (4天)
  ▼
Phase 5 ─── 客户端 WS 模式 (2天)
  └─ ★ 完整功能，实时推送
```

### 4.2 详细任务拆分

#### Phase 0 — 项目骨架

| 任务 | 产出 |
|------|------|
| 创建 workspace + 3 crates 空壳 | `cargo build` 通过 |
| `cs-core` 定义协议类型 | `ClipPayload`, `ClipContent`, `FileEntry` |
| `cs-core` 定义配置结构 | `Config`, `SyncMode` |
| `cs-core` 定义同步引擎 trait | `SyncEngine` trait |
| 工具函数 | `sha256()`, 文件读写 |

#### Phase 1 — Windows 剪贴板

| 任务 | 产出 |
|------|------|
| 剪贴板监控 | 专有线程 + `AddClipboardFormatListener` |
| 文本读取 | CF_UNICODETEXT |
| 图片读取 | CF_DIBV5 → image crate → PNG bytes |
| 文件读取 | CF_HDROP 枚举文件路径 + 读取内容 |
| 文本写入 | `arboard::set_text()` |
| 图片写入 | `arboard::set_image()` + PNG→CF_DIBV5 |
| 文件写入 | 临时目录重建 + HDROP 构造 |
| SHA256 去重 | clip_id 防回音环 |
| 日志输出 | `tracing-subscriber` 终端日志 |

#### Phase 2 — WebDAV 同步引擎

| 任务 | 产出 |
|------|------|
| HTTP 客户端 | reqwest + Basic Auth |
| PROPFIND 探测 | 检查 /SyncClipboard.json 是否存在 |
| 轮询循环 | tokio 定时器 + GET + hash 比对 |
| 上传文本 | PUT /SyncClipboard.json |
| 上传文件 | PUT /file/{name} |
| 下载文件 | GET /file/{name} |
| 可配置轮询间隔 | config.json `poll_interval_secs` |
| 重试/退避 | 失败后 3s → 6s → 12s → max |

#### Phase 3 — 系统托盘

| 任务 | 产出 |
|------|------|
| tray-icon 托盘 | 原生 Win32 托盘图标 |
| 状态图标切换 | 连接/断开/暂停 三态 |
| 右键菜单 | 状态显示 + 暂停/恢复 + 退出 |
| 配置文件读写 | `%APPDATA%/clipsync/config.json` |
| 开机自启 | 注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` |
| 优雅退出 | `ctrl-c` + 托盘 Destroy |

#### Phase 4 — ClipSync 服务端

| 任务 | 产出 |
|------|------|
| Axum 骨架 | REST + WS 双路由 |
| REST /SyncClipboard.json | GET/PUT 读写当前剪贴板 |
| REST /file/{name} | GET/PUT 文件传输 |
| WebDAV 兼容 | PROPFIND / MKCOL 路由 |
| WebSocket /ws | 升级 + 消息收发 |
| 广播 | 排除发送者广播给所有 WS 客户端 |
| SQLite | 设备注册 + 离线消息队列 |
| Basic Auth | `Authorization: Basic base64(u:p)` |
| 健康检查 | GET /health、GET /api/time |

#### Phase 5 — 客户端 WS 模式

| 任务 | 产出 |
|------|------|
| WS 连接 + 鉴权 | 发送 Auth 消息 |
| ClipSyncEngine | 实现 SyncEngine trait |
| WS 发送剪贴板 | 发送 ClientMsg::ClipSync |
| WS 接收广播 | 过滤 source_device_id |
| 文件走 REST | 大文件走 HTTP GET/PUT |
| 断连重连 | 指数退避 |
| 模式切换 | config `mode: "clipsync" \| "webdav"` |

### 4.3 目录结构（最终）

```
clipsync/
├── Cargo.toml
├── rust-toolchain.toml
├── .github/workflows/ci.yml
├── README.md
│
├── crates/
│   ├── cs-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── protocol.rs         # 消息类型
│   │       ├── config.rs           # 配置
│   │       └── util.rs             # sha256 等实用函数
│   │
│   ├── cs-server/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       ├── routes/mod.rs
│   │       ├── routes/sync_profile.rs
│   │       ├── routes/file.rs
│   │       ├── routes/webdav.rs
│   │       ├── routes/health.rs
│   │       ├── ws/mod.rs
│   │       ├── ws/handler.rs
│   │       ├── ws/session.rs
│   │       ├── db/mod.rs
│   │       └── auth.rs
│   │
│   ├── cs-client-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── clipboard/mod.rs
│   │       ├── clipboard/monitor.rs
│   │       ├── clipboard/reader.rs
│   │       ├── clipboard/writer.rs
│   │       ├── sync/mod.rs
│   │       ├── sync/engine.rs
│   │       ├── sync/webdav.rs
│   │       ├── sync/clipsync.rs
│   │       ├── image.rs
│   │       └── hash.rs
│   │
│   └── cs-tray/
│       ├── Cargo.toml
│       ├── build.rs
│       └── src/
│           ├── main.rs
│           └── tray.rs
│
└── resources/
    ├── icons/
    │   ├── connected.ico
    │   ├── disconnected.ico
    │   └── paused.ico
    └── default_config.json
```

---

## 5. 依赖清单

### 5.1 生产依赖

```toml
[dependencies]
# 异步
tokio = { version = "1", features = ["full"] }
futures-util = "0.3"

# 序列化
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# 日志
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# 客户端特有
clipboard-win = { version = "5", features = ["monitor"] }
arboard = "3"
image = "0.25"
tray-icon = "0.19"
reqwest = { version = "0.12", features = ["json"] }

# 服务端特有
axum = { version = "0.8", features = ["ws"] }
tower-http = { version = "0.6", features = ["cors", "trace"] }
rusqlite = { version = "0.32", features = ["bundled"] }

# 通用
sha2 = "0.10"
base64 = "0.22"
anyhow = "1"
thiserror = "2"
```

### 5.2 开发依赖

```toml
[dev-dependencies]
tokio-tungstenite = "0.24"   # WS 客户端测试
tungstenite = "0.24"
reqwest = { version = "0.12", features = ["blocking"] }
tempfile = "3"
```

---

## 6. 与 SyncClipboard 对比

| 维度 | SyncClipboard | ClipSync | 优势方 |
|------|-------------|----------|--------|
| 运行时依赖 | .NET 8 Runtime (~80MB) | 单文件，零依赖 | ClipSync |
| 服务端镜像 | ~100MB (ASP.NET) | ~15MB (Axum) | ClipSync |
| Windows 监控 | WinRT 事件驱动 | 等同 | 持平 |
| 实时推送 | SignalR（成熟） | 自定义 WS（手写） | SyncClipboard |
| REST API 兼容 | WebDAV 完整 | GET/PUT+PROPFIND 够用级 | 持平 |
| 第三方客户端 | 已验证 iOS/Android | 同等级兼容 | 持平 |
| 图片处理 | Magick.NET（全格式） | image crate（基础） | SyncClipboard |
| 跨平台桌面 | Avalonia 三端 | 仅 Windows | SyncClipboard |
| 剪贴板历史 | 完整 SQLite | MVP 无 | SyncClipboard |
| 部署复杂度 | 高 | 低 | ClipSync |
| 代码可控性 | 第三方维护 | 完全掌控 | ClipSync |

---

## 7. 风险和缓解措施

| 风险 | 概率 | 影响 | 缓解措施 |
|------|------|------|----------|
| Rust 生态 crate 不成熟 | 中 | 功能受限 | 关键路径用成熟 crate（tokio/axum），新 crate 评估后再用 |
| 图片格式兼容问题 | 高 | 部分图片无法同步 | MVP 聚焦常见格式，FFmpeg 留后续 |
| WebDAV 服务差异 | 中 | 某些服务兼容问题 | 按标准 WebDAV 实现，针对 Nextcloud/AList 验证 |
| 回音环未能完全防护 | 中 | 死循环 | seq_num + SHA256 + 来源设备 ID 三重防御 |
| Android 10+ 剪贴板限制 | 高 | 后台无法读取 | 移动端用前台服务/通知权限，或手动触发同步 |

---

## 8. 验收标准

MVP 验证场景：

1. **文本同步**：设备 A 复制 "Hello"，设备 B 粘贴得到 "Hello"（< 3s）
2. **图片同步**：设备 A 复制图片（截屏/网页图片），设备 B 粘贴得到原图
3. **文件同步**：设备 A 复制文件，设备 B 粘贴得到文件
4. **WebDAV 模式**：两台 Windows 机器都连上同一个 Nextcloud，相互同步
5. **托盘运行**：程序后台运行，托盘显示状态，右键菜单操作正常
6. **配置持久化**：修改 config.json 后重启应用，生效

---

> 版本：v1.0
> 日期：2026-06-05
> 状态：策划阶段
