# Planning Draft: LiteClipSync WebDAV 支持

## 用户需求
- Windows 和 Android 客户端新增 WebDAV 模式作为备选后端
- 配置时二选一：Server URL（现有 WS 模式）或 WebDAV URL
- 协议格式与 SyncClipboard 兼容
- 冲突策略：Last Write Wins（时间戳 + hash）
- 轮询间隔 3s，可配置
- 目标服务器：Nextcloud、AList、坚果云

## 技术决策
- **协议格式**：采用 SyncClipboard 的 `SyncClipboard.json` + `file/` 目录结构
- **为什么兼容 SyncClipboard**：无更好的方案；格式与我们 ProfileDto 几乎一致；互操作生态大
- **冲突解决**：LWW，用 SHA-256 hash 做变更检测，不依赖 ETag
- **WebDAV 方法**：仅 PUT/GET/MKCOL（不依赖 LOCK、不依赖 ETag）
- **轮询**：3s 默认，可配置 2s/3s/5s/10s
- **服务器兼容性**：Nextcloud ✅、AList ✅、坚果云 ✅（限速但够用）、Seafile ❌

## 服务器兼容性影响
| 服务器 | PROPFIND | MKCOL | ETag | LOCK | 我们的用了没 | 备注 |
|---|---|---|---|---|---|---|
| Nextcloud | ✅ | ✅ | ✅ | ⚠️假 | 没用 | 最佳 |
| AList | ✅ | ✅ | ⚠️弱 | ✅ | 没用 | ETag 弱但无影响 |
| 坚果云 | ✅ | ✅ | ✅ | ⚠️限 | 没用 | 限速 1MB/s |
| Seafile | ❓bug | ⚠️ | ⚠️ | ❌ | 没用 | 不推荐 |

## SyncClipboard 协议格式
```json
{
  "type": "Text",          // Text | Image | File | Group
  "hash": "SHA-256 hex",  // 可选，空串等同 null
  "text": "string",       // Text 类型必填；Image/File 预览字符串
  "hasData": true,         // Image/File 必填
  "dataName": "string",   // hasData=true 时文件名，对应 /file/{dataName}
  "size": 0                // 可选，字节数
}
```

文件存储路径：
- 元数据：`/SyncClipboard.json`（PUT/GET）
- 文件数据：`/file/{dataName}`（PUT/GET）
- 初始化：`MKCOL /` + `MKCOL /file`（忽略 405/409）

## 与我们现有 ProfileDto 的差异
| 字段 | 我们 ProfileDto | SyncClipboard.json | 映射 |
|---|---|---|---|
| 类型 | `content_type` (serde rename "type") | `type` | 直接用 `type` |
| hash | `hash` | `hash` | 相同 |
| 文本 | `text` | `text` | 相同 |
| 有文件 | `has_data` (serde rename "has_data") | `hasData` | 改用 camelCase |
| 文件名 | `data_name` (serde rename "data_name") | `dataName` | 改用 camelCase |
| 大小 | `size` | `size` | 相同 |

**结论**：只需将 serde rename 从 snake_case 改为 camelCase，或为 WebDAV 模式添加映射层。

## Scope
- INCLUDE: Windows 客户端 WebDAV 后端
- INCLUDE: Android 客户端 WebDAV 后端
- INCLUDE: 配置 UI 切换 Server/WebDAV 模式
- INCLUDE: WebDAV 目录初始化（MKCOL）
- INCLUDE: 轮询同步（3s 默认，可配置）
- INCLUDE: 文件上传/下载（流式）
- INCLUDE: Last Write Wins 冲突策略
- EXCLUDE: 服务端 WebDAV 模式（服务端只做自有协议）
- EXCLUDE: ETag/LOCK 支持
- EXCLUDE: 剪贴板历史记录（WebDAV 模式下）