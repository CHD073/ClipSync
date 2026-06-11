# ClipSync

> 自托管跨设备剪贴板实时同步。一次复制，Windows / Android / Server 全端同步。

---

## 快速开始

**服务端**（Linux，Rust 1.70+）：

```bash
git clone https://github.com/CHD073/ClipSync.git && cd ClipSync/clipsync-server
cargo build --release
export CLIPSYNC_TOKEN="your_secret_token"
./target/release/clipsync-server
```

**Windows 客户端**（Windows 10/11，MSVC toolchain）：

```bash
cd clipsync-windows && cargo build --release
# 双击 clipsync.exe，托盘图标出现即运行
```

**Android 客户端**（Android 9+，JDK 17）：

```bash
cd clipsync-android && ./gradlew assembleDebug
# 安装 APK → 填入 Server URL → 在 Shizuku 中授权 → 点 Start
```

> Android 后台同步依赖 [Shizuku](https://shizuku.rikka.app/) 框架。

---

## 架构

```
Windows ── WebSocket / HTTPS ──►  Server (Rust/Axum)  ◄── WebSocket ──  Android (Kotlin)
                                     │
                               SQLite + 文件存储
```

| 端 | 语言 | 运行形态 |
|----|------|----------|
| Server | Rust | Linux 服务（systemd） |
| Windows | Rust | 系统托盘，无窗口 |
| Android | Kotlin | 前台 Service + Shizuku |

---

## 功能

### 剪贴板同步

| 类型 | 通道 | 上限 |
|------|------|------|
| 短文本 | WebSocket 直传 | 10 MB |
| 长文本 / 文件 | HTTP 上传 + WS 通知 | 无限制 |

### 同步模式

- **Auto Sync** — 剪贴板变化自动推送（可独立开关）
- **手动 Upload** — 托盘菜单 / 热键 / App 按钮
- **手动 Download** — 托盘菜单 / 热键 / App 按钮

### Windows 端

- 🟢🟡🔴 三色托盘图标 + 实时 tooltip
- 全局热键：`Ctrl+Shift+C` 推 / `Ctrl+Shift+V` 拉 / `Ctrl+Alt+V` 切
- 托盘菜单：Upload、Download、Auto-Sync、Settings、Open Log、Restart、Quit
- **中英双语** — Settings → `中文`/`English`，持久化到 `config.toml`
- 开机自启、单实例保护、优雅退出

### Android 端

- Compose 单页 UI：Shizuku 状态卡 → Server 配置 → 同步控制 → 日志面板
- **后台同步** — 通过 Shizuku UserService 以 shell UID 直接调用 `IClipboard`
- **前台同步** — `ClipboardManager` 原生 API
- 中英自动切换（随系统语言）

---

## 协议

### WebSocket（JSON，Basic Auth）

| 方向 | 消息 | 载荷 |
|------|------|------|
| → 服务端 | `Auth` | `token` + `device_id` + `name` |
| → 服务端 | `ClipSync` | `ProfileDto` |
| ← 客户端 | `AuthOk` / `AuthError` | `device_id` / `reason` |
| ← 客户端 | `ClipBroadcast` | `ProfileDto` + 来源设备 |
| ← 客户端 | `Backlog` | 离线消息列表 |

### REST API

| 方法 | 路径 | 说明 |
|------|------|------|
| `GET/PUT` | `/profile/latest` | 获取 / 更新最新剪贴板 |
| `GET/PUT` | `/file/{name}` | 文件上传下载 |
| `GET` | `/health` | 健康检查 |

### ProfileDto

```json
{
    "type": "Text",
    "hash": "SHA-256",
    "text": "内容",
    "has_data": true,
    "data_name": "filename",
    "size": 1234
}
```

---

## 配置

### 服务端（环境变量）

| 变量 | 默认 | 说明 |
|------|------|------|
| `CLIPSYNC_PORT` | `8765` | |
| `CLIPSYNC_TOKEN` | `clipsync` | ⚠️ 生产必须改 |
| `CLIPSYNC_STORAGE_PATH` | `./data` | DB + 文件 |
| `CLIPSYNC_TLS_CERT_PATH` / `KEY_PATH` | — | 同时设才启 HTTPS |

### Windows（config.toml）

| 参数 | 类型 | 默认 |
|------|------|------|
| `server_url` | string | — |
| `token` | string | — |
| `device_name` | string | 主机名 |
| `auto_sync` | bool | `true` |
| `auto_sync_max_bytes` | int | `10485760` |
| `autostart` | bool | `false` |
| `language` | string | `"en"` |

### Android（App 内）

| 参数 | 默认 |
|------|------|
| Server URL | — |
| Token | `clipsync` |
| Auto Sync | 开 |

---

## 部署

### systemd

```ini
# /etc/systemd/system/clipsync.service
[Unit]
Description=ClipSync Server
After=network.target

[Service]
Type=simple
ExecStart=/opt/clipsync/clipsync-server
Environment=CLIPSYNC_TOKEN=xxx
Environment=CLIPSYNC_STORAGE_PATH=/var/lib/clipsync
Restart=always
User=clipsync

[Install]
WantedBy=multi-user.target
```

```bash
sudo useradd -r clipsync
sudo mkdir -p /var/lib/clipsync && sudo chown clipsync:clipsync /var/lib/clipsync
sudo cp target/release/clipsync-server /opt/clipsync/
sudo systemctl enable --now clipsync
```

### Android Shizuku 配置

1. 安装 [Shizuku App](https://shizuku.rikka.app/)
2. 启动 Server：`adb shell /data/app/~~..~~/lib/arm64/libshizuku.so`
3. 打开 ClipSync → Shizuku 中授权 → 卡片变绿
4. 填入 Server URL → Start

---

## 项目结构

```
├── clipsync-server/     Rust 服务端 (Axum + SQLite + WS)
├── clipsync-windows/    Rust Windows 托盘客户端
└── clipsync-android/    Kotlin Android 客户端 (Compose + Shizuku)
```

---

## 安全

| 风险 | 建议 |
|------|------|
| 默认 Token | 生产设置 `CLIPSYNC_TOKEN` 为随机长串 |
| 明文传输 | 配置 TLS 证书 |
| 无请求大小限制 | nginx 前置 `client_max_body_size` |
| 无速率限制 | nginx / cloudflare 限流 |

---

## 故障排除

**服务端启动失败** — `ss -tlnp | grep 8765` 检查端口

**Win 托盘不显示** — 检查单实例互斥锁，RDP 下托盘可能隐藏

**Android 后台不工作** — Shizuku 卡片必须绿色；必要时重启 Shizuku Server

**PC 收不到** — 确认同网段、同 Server、Auto Sync 开启

---

## 许可证

[MIT](LICENSE)
