# ClipSync

跨设备剪贴板同步工具 — 自托管，实时同步。

## 架构

```
┌─────────────┐     WebSocket     ┌──────────────┐
│  Windows     │ ◄──────────────► │  Server       │
│  (Client)    │     REST API     │  (CentOS VM)  │
└─────────────┘                   └──────┬───────┘
                                         │
                                ┌────────▼───────┐
                                │  Android        │
                                │  (planned)      │
                                └─────────────────┘
```

- **Server** — Rust / Axum，WebSocket 实时推送 + REST API
- **Windows** — Rust，系统托盘图标，无窗口，全局热键
- **Android** — 纯 Kotlin（待实现）

## 功能

- 同步文本、图片、文件至所有在线设备
- 全局热键：Ctrl+Shift+C（复制并同步）、Ctrl+Shift+V（同步并粘贴）、Ctrl+Alt+V（开关自动同步）
- 自动同步 / 手动上传下载
- 离线设备上线后自动推送积压内容
- WebDAV 支持（规划中）

## 快速开始

### 服务端

```bash
# 在 CentOS VM 上运行
/path/to/clipsync-server

# 默认监听 0.0.0.0:8765
```

### Windows 客户端

1. 下载 `clipsync.exe`
2. 同目录放置 `config.toml`（见下方配置）
3. 双击运行（托盘图标运行，无窗口）

```toml
# config.toml
server_url = "http://192.168.1.100:8765"
token = "your_token"
device_name = "My PC"
auto_sync = true
```

## 配置说明

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `server_url` | — | 服务端地址 |
| `token` | — | 认证令牌 |
| `device_name` | 主机名 | 设备名称 |
| `auto_sync` | `true` | 启动时自动同步 |
| `auto_sync_max_bytes` | 10485760 | 自动同步最大字节数（超限需手动） |
| `ws_inline_max_bytes` | 1048576 | WebSocket 内联传输上限 |
| `http_timeout_secs` | 180 | HTTP 请求超时（秒） |
| `hotkey_copy_sync` | `Ctrl+Shift+C` | 复制并同步热键 |
| `hotkey_sync_paste` | `Ctrl+Shift+V` | 同步并粘贴热键 |
| `hotkey_toggle_auto` | `Ctrl+Alt+V` | 开关自动同步热键 |

## 构建

```bash
# 服务端
cd clipsync-server
cargo build --release

# Windows 客户端
cd clipsync-windows
cargo build --release
```

## 协议

[MIT](LICENSE)
