# LiteClipSync

> Self-hosted cross-device clipboard sync. Copy once, available everywhere — Windows / Server / Android.

[中文](../README.md) | English

---

## Features

- **Real-time clipboard sync** — text and files across Windows, Android, and any device via self-hosted server
- **Windows tray client** — zero-window background operation, global hotkeys, auto-start
- **Android background sync** — via [Shizuku](https://shizuku.rikka.app/) UserService, bypasses Android 10+ restrictions
- **Echo protection** — hash-based deduplication with cooldown
- **Offline backlog** — missed clips delivered on reconnect
- **Bilingual UI** — Chinese/English auto-detect (Android), manual switch (Windows)
- **Streaming transfers** — large files don't OOM
- **Self-hosted** — runs on your own Linux server, no third-party cloud

---

## Architecture

```
Windows ── WebSocket / HTTPS ──►  Server (Rust/Axum)  ◄── WebSocket ──  Android (Kotlin)
                                     │
                               SQLite + file storage
```

| Component | Language | Key Dependencies |
|-----------|----------|------------------|
| Server | Rust | Axum, tokio-tungstenite, rusqlite, rustls |
| Windows Client | Rust | tray-icon, arboard, reqwest |
| Android Client | Kotlin | OkHttp, Compose, Shizuku |

---

## Quick Start

```bash
# Server
git clone https://github.com/CHD073/ClipSync.git && cd LiteClipSync/liteclipsync-server
cargo build --release
export LITECLIPSYNC_TOKEN="your_secret_token"
./target/release/liteclipsync-server

# Windows
cd liteclipsync-windows && cargo build --release
# Double-click liteclipsync.exe, edit config.toml with server_url + token

# Android
cd liteclipsync-android && ./gradlew assembleDebug
# Install APK → set Server URL → authorize in Shizuku → Start
```

---

## Protocol

### WebSocket (JSON, Basic Auth)

| Direction | Message | Payload |
|-----------|---------|---------|
| Client→Server | `Auth` | `token` + `device_id` + `name` |
| Client→Server | `LiteClipSync` | `ProfileDto` |
| Server→Client | `AuthOk` / `AuthError` | `device_id` / `reason` |
| Server→Client | `ClipBroadcast` | `ProfileDto` + source device |
| Server→Client | `Backlog` | Offline message list |

### REST API

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| `GET/PUT` | `/profile/latest` | Basic | Get / update latest clip |
| `GET/PUT` | `/file/{name}` | Basic | File upload / download |
| `GET` | `/health` | None | Health check |
| `GET` | `/api/time` | None | Server timestamp |

### ProfileDto

```json
{
    "type": "Text",
    "hash": "SHA-256",
    "text": "content",
    "has_data": true,
    "data_name": "filename",
    "size": 1234
}
```

---

## Server

### Requirements

- Linux (any distro)
- Rust 1.70+
- Open port (default 8765)

### Deployment

```bash
cargo build --release

# Environment variables
export LITECLIPSYNC_PORT=8765
export LITECLIPSYNC_TOKEN="your_random_secret"
export LITECLIPSYNC_STORAGE_PATH="/opt/liteclipsync/data"

# Optional: HTTPS
export LITECLIPSYNC_TLS_CERT_PATH="/path/to/fullchain.pem"
export LITECLIPSYNC_TLS_KEY_PATH="/path/to/privkey.pem"

./target/release/liteclipsync-server
```

### systemd

```ini
[Unit]
Description=LiteClipSync Server
After=network.target

[Service]
Type=simple
ExecStart=/opt/liteclipsync/liteclipsync-server
Environment=LITECLIPSYNC_TOKEN=xxx
Environment=LITECLIPSYNC_STORAGE_PATH=/var/lib/liteclipsync
Restart=always
User=liteclipsync

[Install]
WantedBy=multi-user.target
```

```bash
sudo useradd -r liteclipsync
sudo mkdir -p /var/lib/liteclipsync && sudo chown liteclipsync:liteclipsync /var/lib/liteclipsync
sudo cp target/release/liteclipsync-server /opt/liteclipsync/
sudo systemctl enable --now liteclipsync
```

### Data Storage

- Database: `{LITECLIPSYNC_STORAGE_PATH}/liteclipsync.db` (SQLite, WAL mode)
- Files: `{LITECLIPSYNC_STORAGE_PATH}/files/`
- History: retained for `LITECLIPSYNC_MAX_HISTORY_DAYS` days (default 7)

---

## Windows Client

### Requirements

- Windows 10/11 64-bit
- Rust MSVC toolchain + Visual Studio Build Tools

### Usage

1. Place `liteclipsync.exe` anywhere
2. Run once to auto-generate `config.toml`
3. Edit `config.toml` — set `server_url` and `token`
4. Double-click — tray icon appears

### Features

- Tray icon: 🟢 Connected / 🔴 Disconnected / 🔵 Syncing
- Global hotkeys: `Ctrl+Shift+C` push / `Ctrl+Shift+V` pull / `Ctrl+Alt+V` toggle
- Tray menu: Upload, Download, Auto-Sync toggle, Settings, Open Log, Restart, Quit
- **Language switch**: Settings → `中文`/`English`, persisted to `config.toml`
- Auto-start, single-instance guard, graceful shutdown

### Configuration

`config.toml` — placed next to `liteclipsync.exe`:

```toml
server_url = "http://192.168.1.100:8765"
token = "your_token"
device_name = "MyPC"
auto_sync = true
language = "en"   # "en" or "zh"
```

---

## Android Client

### Requirements

- Android 9.0+ (minSdk 28)
- JDK 17
- Android SDK (compileSdk 35)
- [Shizuku App](https://shizuku.rikka.app/) for background sync

### Shizuku Setup

1. Install Shizuku App
2. Start Shizuku Server via ADB:
   ```bash
   adb shell /data/app/~~XXXX==/moe.shizuku.privileged.api-XXXX==/lib/arm64/libshizuku.so
   ```
3. Open LiteClipSync → Authorize in Shizuku → Card turns green
4. Set Server URL + Token → Start

> **Note:** Restart Shizuku Server after device reboot.

### Background Sync Principle

```
User copies text
  ↓
Foreground Service keeps process alive
  ↓
If cm.primaryClip returns stale → ShizukuShell.getText()
  ↓
Shizuku UserService (UID 2000/shell)
  ↓
Reflection: IClipboard.getPrimaryClip("com.android.shell")
  ↓
Fresh ClipData → extract text → WS push
```

---

## Configuration Reference

### Server (Environment Variables)

| Variable | Default | Description |
|----------|---------|-------------|
| `LITECLIPSYNC_PORT` | `8765` | Listen port |
| `LITECLIPSYNC_TOKEN` | `liteclipsync` | ⚠️ Must change in production |
| `LITECLIPSYNC_STORAGE_PATH` | `./data` | DB + file storage |
| `LITECLIPSYNC_MAX_HISTORY_DAYS` | `7` | History retention |
| `LITECLIPSYNC_TLS_CERT_PATH` | — | TLS cert path |
| `LITECLIPSYNC_TLS_KEY_PATH` | — | TLS key path |

### Windows Client (config.toml)

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `server_url` | string | — | Server address |
| `token` | string | — | Auth token |
| `device_name` | string | hostname | Display name |
| `auto_sync` | bool | `true` | Enable auto-sync |
| `auto_sync_max_bytes` | int | `10485760` | Max auto-sync size |
| `autostart` | bool | `false` | Auto-start with Windows |
| `language` | string | `"en"` | UI language: `"en"` / `"zh"` |

### Android Client (SharedPreferences)

| Parameter | Default | Description |
|-----------|---------|-------------|
| `server_url` | — | Server address |
| `token` | `liteclipsync` | Auth token |
| `auto_sync` | `true` | Background polling |

---

## Build

```bash
# Server
cd liteclipsync-server && cargo build --release

# Windows
cd liteclipsync-windows && cargo build --release

# Android
cd liteclipsync-android && ./gradlew assembleDebug
```

---

## Security

| Risk | Mitigation |
|------|------------|
| Default token | Set `LITECLIPSYNC_TOKEN` to random string |
| Plain HTTP | Enable TLS via `TLS_CERT_PATH` / `TLS_KEY_PATH` |
| No rate limiting | Use nginx / cloudflare |
| SQLite no encryption | Restrict file permissions |

---

## Troubleshooting

**Server fails to start** — `ss -tlnp | grep 8765` to check port

**Windows tray not showing** — check single-instance mutex; RDP may hide tray

**Android background sync not working** — Shizuku card must be green; restart Shizuku Server

**PC not receiving** — verify same network, same server, Auto Sync enabled

---

## Project Structure

```
├── liteclipsync-server/     Rust server (Axum + SQLite + WS)
├── liteclipsync-windows/    Rust Windows tray client
└── liteclipsync-android/    Kotlin Android client (Compose + Shizuku)
```

---

## License

[MIT](../LICENSE)
