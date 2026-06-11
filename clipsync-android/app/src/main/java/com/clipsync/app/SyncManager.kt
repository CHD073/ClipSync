package com.clipsync.app

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.util.Log
import kotlinx.coroutines.*
import java.security.MessageDigest

class SyncManager(private val ctx: Context) {
    companion object {
        private const val MAX_CLIP_BYTES = 1_000_000
    }
    val cfg = (ctx.applicationContext as ClipSyncApp).config
    val http = HttpApi(cfg)
    private val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
    private var ws: WsClient? = null
    private var lastHash: String? = null
    private var pendingHash: String? = null
    private var pendingText: String? = null
    private var pendingSize: Long = 0
    private var job: Job? = null
    private var cycling = false
    @Volatile var connected = false
    private var autoSyncEnabled = cfg.autoSync
    private var cooldownUntil = 0L
    private var clipChanged = false

    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        clipChanged = true
    }

    val running get() = cycling
    fun setAutoSync(enabled: Boolean) { autoSyncEnabled = enabled; cfg.autoSync = enabled }

    fun start() {
        if (cycling) return
        cm.addPrimaryClipChangedListener(clipListener)
        cycling = true; job = CoroutineScope(Dispatchers.IO).launch { run() }
    }
    fun stop() { cycling = false; job?.cancel(); ws?.close(); ws = null; connected = false; cfg.connected = false; cm.removePrimaryClipChangedListener(clipListener) }

    private suspend fun run() {
        while (cycling) {
            ws?.close(); connected = false; cfg.connected = false
            if (!cycling) break
            checkClipboard(null)
            val w = WsClient(cfg).also { ws = it }
            val r = w.connect()
            if (r.isFailure) { delay(500); continue }
            connected = true; cfg.connected = true
            if (pendingText != null) {
                sendClip(w, pendingHash!!, pendingText!!, pendingSize)
                pendingText = null
            }
            while (cycling && w.connected) {
                try {
                    pollWs(w)
                    if (autoSyncEnabled) checkClipboard(w)
                    delay(100)
                } catch (e: Exception) {
                    Log.e("ClipSync", "sync crash: ${e.message}", e)
                }
            }
            Log.e("ClipSync", "sync: disconnected, reconnecting...")
            connected = false; cfg.connected = false
        }
    }

    private suspend fun sendClip(w: WsClient, h: String, txt: String, sz: Long) {
        cooldownUntil = System.currentTimeMillis() + 200
        lastHash = h
        if (sz <= cfg.autoSyncMaxBytes) {
            w.send(buildClipSync(ProfileDto("Text", h, text = txt, size = sz), cfg.deviceId))
        } else {
            val name = "${safePrefix(txt)}_${h.take(8)}.txt"
            http.upload(name, txt.toByteArray())
            w.send(buildClipSync(ProfileDto("Text", h, hasData = true, dataName = name, size = sz), cfg.deviceId))
        }
        cfg.lastSyncTime = java.text.SimpleDateFormat("HH:mm:ss", java.util.Locale.getDefault()).format(java.util.Date())
    }

    suspend fun uploadText(text: String): String {
        val h = sha256(text.toByteArray()); lastHash = h
        val wl = ws; if (wl == null || !wl.connected) return "WS not connected"
        if (text.toByteArray().size <= cfg.autoSyncMaxBytes) {
            wl.send(buildClipSync(ProfileDto("Text", h, text = text, size = text.length.toLong()), cfg.deviceId))
            return "sent (${text.length} chars)"
        } else {
            val name = "${safePrefix(text)}_${h.take(8)}.txt"
            val r = http.upload(name, text.toByteArray())
            if (r.isFailure) return "upload failed: ${r.exceptionOrNull()?.message}"
            wl.send(buildClipSync(ProfileDto("Text", h, hasData = true, dataName = name, size = text.length.toLong()), cfg.deviceId))
            return "uploaded as file"
        }
    }

    suspend fun fetchLatest(): ProfileDto? {
        return try {
            val c = okhttp3.OkHttpClient.Builder().connectTimeout(10, java.util.concurrent.TimeUnit.SECONDS).build()
            val r = okhttp3.Request.Builder().url("${cfg.apiBase}/profile/latest").get()
                .header("Authorization", cfg.authHeader).build()
            val resp = c.newCall(r).execute()
            if (!resp.isSuccessful) { Log.w("ClipSync", "fetch HTTP ${resp.code}"); return null }
            val json = resp.body?.string() ?: return null
            val m = com.google.gson.Gson().fromJson(json, Map::class.java) as Map<String, Any?>
            com.google.gson.Gson().fromJson(com.google.gson.Gson().toJson(m["payload"]), ProfileDto::class.java)
        } catch (e: Exception) { Log.w("ClipSync", "fetch error: ${e.message}"); null }
    }

    suspend fun uploadFromUri(uri: android.net.Uri, cr: android.content.ContentResolver,
                              name: String, totalSize: Long,
                              onProgress: (Long, Long) -> Unit): String {
        val h = sha256(name.toByteArray()); lastHash = h
        val r = http.uploadStream(name, uri, cr, totalSize, onProgress)
        if (r.isFailure) return "upload failed: ${r.exceptionOrNull()?.message}"
        val wl = ws; if (wl == null || !wl.connected) return "uploaded but WS disconnected"
        wl.send(buildClipSync(ProfileDto("File", h, hasData = true, dataName = name, size = totalSize), cfg.deviceId))
        return "uploaded: $name"
    }

    suspend fun downloadToTempFile(dataName: String, onProgress: (Long, Long) -> Unit): java.io.File? {
        val f = java.io.File(ctx.cacheDir, "dl_$dataName")
        val r = http.downloadToFile(dataName, f, onProgress)
        return if (r.isSuccess) f else null
    }

    private suspend fun pollWs(w: WsClient) {
        var msg: ServerMsg? = w.incoming.tryReceive().getOrNull()
        while (msg != null) {
            when (msg) {
                is ServerMsg.ClipBroadcast -> {
                    if (msg.sourceDeviceId != cfg.deviceId && msg.payload.size <= cfg.autoSyncMaxBytes) {
                        applyRemote(msg.payload)
                        cfg.lastSyncTime = java.text.SimpleDateFormat("HH:mm:ss", java.util.Locale.getDefault()).format(java.util.Date())
                        cfg.lastSyncFrom = msg.sourceDeviceName.ifEmpty { msg.sourceDeviceId }
                    }
                }
                is ServerMsg.Backlog -> for (e in msg.entries) if (e.size <= cfg.autoSyncMaxBytes) applyRemote(e)
                else -> {}
            }
            msg = w.incoming.tryReceive().getOrNull()
        }
    }

    private suspend fun checkClipboard(w: WsClient?) {
        if (!clipChanged && System.currentTimeMillis() < cooldownUntil) return
        clipChanged = false
        val item = cm.primaryClip?.getItemAt(0)
        var clipText = item?.text?.toString()
        if (clipText != null && sha256(clipText.toByteArray()) == lastHash && ShizukuShell.available()) {
            val fresh = ShizukuShell.getText()
            if (fresh != null && fresh.isNotEmpty() && fresh != clipText) {
                Log.e("ClipSync", "stale cm=[${clipText}] → shizuku=[${fresh}]")
                clipText = fresh
            }
        }
        if (clipText == null && ShizukuShell.available()) {
            clipText = ShizukuShell.getText()
        }
        if (clipText == null || clipText.isEmpty()) return
        Log.d("ClipSync", "check: got ${clipText.length} chars")
        if (clipText.toByteArray().size > MAX_CLIP_BYTES) return
        val h = sha256(clipText.toByteArray())
        if (h == lastHash) return
        Log.e("ClipSync", "check: SEND")
        if (w != null) sendClip(w, h, clipText, clipText.length.toLong())
        else { lastHash = h; pendingHash = h; pendingText = clipText; pendingSize = clipText.length.toLong() }
    }

    suspend fun applyRemote(p: ProfileDto) {
        cooldownUntil = System.currentTimeMillis() + 200
        when (p.contentType) {
            "Text" -> {
                val txt = if (p.hasData) String(http.download(p.dataName).getOrElse { return }) else p.text
                if (txt.length > MAX_CLIP_BYTES) {
                    Log.w("ClipSync", "applyRemote: text too large (${txt.length} chars), skip")
                    return
                }
                val h = sha256(txt.toByteArray())
                if (h == lastHash) return  // already synced, skip to avoid overwriting local edits
                lastHash = h
                cm.setPrimaryClip(ClipData.newPlainText("ClipSync", txt))
            }
        }
    }

    private fun safePrefix(text: String): String {
        val cleaned = text.replace('\n', ' ').replace('\r', ' ').replace('/', '_').replace('\\', '_')
        val trimmed = cleaned.take(30).trim()
        return trimmed.ifEmpty { "clip" }
    }

    private fun sha256(data: ByteArray): String = MessageDigest.getInstance("SHA-256")
        .digest(data).joinToString("") { "%02x".format(it) }
}
