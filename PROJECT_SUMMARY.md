# ClipSync 项目总结

## 概述

ClipSync 是一个跨设备剪贴板同步工具，支持 PC ↔ 手机（Android）双向同步。通过自建服务端中转，实现不同设备间剪贴板内容的实时同步。

## 架构

```
┌─────────────┐     WebSocket      ┌──────────────┐     WebSocket      ┌──────────────┐
│  Windows    │ ◄────────────────► │   Server     │ ◄────────────────► │  Android     │
│  客户端     │                    │ (Rust/Axum)  │                    │  客户端      │
│             │    HTTP PUT/GET    │              │    HTTP PUT/GET    │              │
│  Rust/Tray  │ ◄────────────────► │  SQLite 存储 │ ◄────────────────► │  Kotlin/Jet  │
│  Tauri托盘  │                    │  文件存储    │                    │  Compose     │
└─────────────┘                    └──────────────┘                    └──────────────┘
```

### 组件

| 组件 | 语言 | 核心依赖 |
|------|------|----------|
| **服务端** | Rust | Axum, tokio-tungstenite, rusqlite, rustls |
| **Windows 客户端** | Rust | tray-icon, tokio-tungstenite, reqwest, arboard |
| **Android 客户端** | Kotlin | OkHttp, Jetpack Compose, Gson, Shizuku API |

## 开发历程

### 1. 核心功能（服务端 + 双向同步）

- WebSocket 实时推送剪贴板变化
- HTTP REST API（上传/下载文件、获取最新剪贴板）
- SQLite 离线 Backlog
- Basic Auth 鉴权
- 可选 TLS 支持（`CLIPSYNC_TLS_CERT_PATH`/`CLIPSYNC_TLS_KEY_PATH`）

### 2. Android 后台剪贴板读取的挑战

这是项目中最困难的部分。Android 10 起系统严格限制后台读取剪贴板，Android 15 进一步收紧。我们尝试了以下方案：

#### ❌ `cmd clipboard get-text`

Android 系统 shell 命令。部分 ROM 不支持（此项目设备上返回 "No shell command implementation"）。

#### ❌ `appops READ_CLIPBOARD allow`

标准绕过方式。Android 15 上即使设置该值，`getPrimaryClip()` 在后台仍返回 null。

#### ❌ `content://clipboard/`

内容提供者方式。Android 不存在该 URI。

#### ❌ AccessibilityService

可捕获复制操作但不通用，只对触发 AccessibilityEvent 的应用有效，且会捕获无关 UI 元素文本。

#### ✅ Shizuku（经过多次尝试）

**最终成功方案**，但整合过程充满陷阱：

### 3. Shizuku 整合的曲折历史

#### 尝试 1：SDK 12.2.0（失败）
- `newProcess()` 可用（公开 but deprecated）
- `pingBinder()` 返回 false
- 原因：SDK 12.x 与 Server 13.6.0 相容？

#### 尝试 2：SDK 13.1.5 + Java 反射包装（失败）
- `newProcess()` 在 13.x 中变为 private（`@RestrictTo(LIBRARY_GROUP)`）
- 即使 Java 包装也无法调用（编译器直接拒绝 private 方法）
- 最终改用 Kotlin 反射 `getDeclaredMethod`

#### 尝试 3：SDK 13.1.5 + UserService Bridge（混合）
- 创建 `IShizukuApplication.Stub` Service 让 Manager 绑定
- 编译出错：接口不匹配

#### 尝试 4：JitPack 构建 SDK 13.6.0（失败）
- JitPack 环境 NDK 许可问题，构建失败

#### 尝试 5：SDK 11.0.3（失败）
- 期望 `ShizukuApiProvider` ContentProvider 存在
- 所有已发布版本都没有 ContentProvider

#### 尝试 6：SDK 13.1.5 + BinderContainer（成功！）
- Manager 通过 `ContentProvider.call("sendBinder", ...)` 传递 binder
- Bundle 中包含 `moe.shizuku.api.BinderContainer` Parcelable
- SDK 中没有这个类，需自行实现
- 关键：Java 实现而非 Kotlin（Kotlin `val binder` 生成 `getBinder()` 方法问题）

**最终链路**：
```
Shizuku Server 13.6.0
  → 启动时扫描 App 的 ContentProvider（com.clipsync.app.shizuku）
  → 调用 ContentProvider.call("sendBinder")
  → Bundle 中包含 BinderContainer → 反序列化 → 取 binder
  → Shizuku.onBinderReceived(binder) → pingBinder() = true
  → ShizukuCompat(反射) → newProcess(["service call clipboard 1 ..."])
  → 读取剪贴板
```

### 4. 关键教训

| 教训 | 详情 |
|------|------|
| **SDK 版本 ≠ Server 版本** | Shizuku SDK 13.1.5（2023-09）与 Manager 13.6.0（2025-05）之间隔了两年，但 Manager 负责协议转换，旧 SDK 理论可用 |
| **Provider 非必需** | Shizuku 13.x 中 ContentProvider 被移除，改用 `V3_SUPPORT` meta-data + `sendBinder` 方式 |
| **Kotlin vs Java 序列化** | `BinderContainer` 用 Kotlin `val` 实现时 `getBinder()` 方法在 Bundle 反序列化中找不到，Java 实现解决 |
| **classLoader 关键** | Bundle 反序列化需要 `extras.classLoader = context.classLoader` 才能找到 app 中定义的 Parcelable 类 |
| **反射绕过限制** | `@RestrictTo` 在 Kotlin 编译器层面拦截，Kotlin 反射 `getDeclaredMethod` + `isAccessible=true` 可绕過 |
| **logcat 调试** | MIUI ROM 会过滤 `Log.d` 日志，需用 `Log.e` + `*:E` 级别查看 |

### 5. 文件结构

```
clipsync-android/
├── app/src/main/java/com/clipsync/app/
│   ├── MainActivity.kt          # UI（Shizuku 状态、Start/Stop、Auto-Sync）
│   ├── SyncManager.kt           # 同步逻辑（WS 连接、剪贴板轮询、收发）
│   ├── WsClient.kt              # OkHttp WebSocket 客户端
│   ├── HttpApi.kt               # HTTP 上传/下载（流式）
│   ├── Protocol.kt              # 协议 DTO + JSON 解析
│   ├── Config.kt                # SharedPreferences 配置
│   ├── ClipSyncApp.kt           # Application 类 + Shizuku listener
│   ├── SyncService.kt           # 前台 Service（通知 + 后台同步）
│   ├── ClipboardShell.kt        # Shizuku 剪贴板读写封装
│   ├── ShizukuCompat.kt         # Kotlin 反射包装 Shizuku.newProcess
│   ├── ShizukuApiProvider.kt    # ContentProvider（接收 binder）
│   └── moe/shizuku/api/
│       └── BinderContainer.java # Parcelable 桥接类（Java）
│
clipsync-server/src/
├── main.rs                      # 入口 + TLS 配置
├── config.rs                    # 配置（环境变量）
├── db.rs                        # SQLite 数据库
├── auth.rs                      # Basic Auth 鉴权
├── ws/
│   ├── handler.rs               # WebSocket 连接处理
│   └── session.rs               # 广播频道
└── routes/
    ├── sync_profile.rs          # 剪贴板 REST API
    ├── file.rs                  # 文件上传/下载
    └── health.rs                # 健康检查

clipsync-windows/src/
├── main.rs                      # 托盘图标 + 热键 + 消息泵
├── sync.rs                      # WebSocket 同步循环
├── client.rs                    # HTTP/WS 客户端
├── clipboard.rs                 # 系统剪贴板读写
├── config.rs                    # TOML 配置
├── protocol.rs                  # 协议 DTO
└── command.rs                   # 命令枚举 + 状态结构
```

### 6. 生产化注意事项

#### 服务端
- 设置 `CLIPSYNC_TOKEN` 环境变量（不要用默认值）
- 配置 `CLIPSYNC_TLS_CERT_PATH`/`CLIPSYNC_TLS_KEY_PATH` 启用 HTTPS
- 建议用 systemd 或 Docker 管理进程生命周期
- 定期备份 `clipsync.db` 数据库

#### Windows 客户端
- 托盘图标常驻系统栏
- 全局热键：`Ctrl+Shift+C`（复制并同步）、`Ctrl+Shift+V`（同步并粘贴）、`Ctrl+Shift+X`（切换自动同步）
- 配置文件：`config.toml` 与 exe 同目录
- 支持自启动

#### Android 客户端
- 需要安装 Shizuku App 并授权（`moe.shizuku.privileged.api`）
- 首次使用需执行一次 `adb shell /path/to/libshizuku.so` 启动 Shizuku Server（重启设备后需重新执行）
- Shizuku 卡片显示绿色"Ready"即正常
- Start 按钮控制 WS 连接
- Auto Sync 开关独立控制后台轮询
- 手动 Upload/Download 仅需 WS 连接

### 7. 技术栈选择理由

| 技术 | 选择理由 |
|------|---------|
| **Rust（服务端/Windows）** | 高性能、内存安全、跨平台、异步生态完善 |
| **Axum** | Rust 生态最活跃的 Web 框架，tower 中间件 |
| **tokio-tungstenite** | 异步 WebSocket，与 tokio 原生集成 |
| **SQLite** | 零配置、单文件、适合小规模部署 |
| **Kotlin/Compose** | Android 官方推荐，声明式 UI |
| **OkHttp** | Android 最佳 HTTP/WS 客户端 |
| **Shizuku** | 唯一能在 Android 15 后台读剪贴板的方案 |
| **tray-icon** | Rust 跨平台托盘图标 |
| **arboard** | Rust 跨平台剪贴板库 |

---

*项目周期：2024-2025 | 核心开发者：MECHREVO*
