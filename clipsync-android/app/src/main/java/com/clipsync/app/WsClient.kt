package com.clipsync.app

import kotlinx.coroutines.*
import kotlinx.coroutines.channels.Channel
import okhttp3.*
import java.util.concurrent.TimeUnit

class WsClient(private val cfg: Config) {
    private val client = OkHttpClient.Builder()
        .connectTimeout(30, TimeUnit.SECONDS).readTimeout(0, TimeUnit.MILLISECONDS).build()

    private var ws: WebSocket? = null
    val incoming = Channel<ServerMsg>(Channel.UNLIMITED)
    var connected = false

    suspend fun connect(connectMs: Long = 10000): Result<String> = withContext(Dispatchers.IO) {
        val l = Listener()
        ws = client.newWebSocket(Request.Builder().url(cfg.wsUrl).build(), l)
        withTimeout(connectMs) { while (!l.done) delay(100) }
        if (l.authOk) Result.success(l.deviceId) else Result.failure(Exception(l.err))
    }

    fun send(text: String) { ws?.send(text) }
    fun close() { ws?.close(1000, null); ws = null; connected = false }

    private inner class Listener : WebSocketListener() {
        var done = false; var authOk = false; var deviceId = ""; var err = ""
        override fun onOpen(ws: WebSocket, resp: Response) { connected = true; ws.send(buildAuth(cfg.token, cfg.deviceId, cfg.deviceName)) }
        override fun onMessage(ws: WebSocket, text: String) {
            val msg = parseServerMsg(text)
            if (msg is ServerMsg.AuthOk) { deviceId = msg.deviceId; authOk = true; done = true }
            else if (msg is ServerMsg.AuthError) { err = msg.reason; done = true }
            else if (msg != null) incoming.trySend(msg)
        }
        override fun onFailure(ws: WebSocket, t: Throwable, resp: Response?) { err = t.message ?: "fail"; done = true; connected = false }
        override fun onClosed(ws: WebSocket, code: Int, reason: String) { connected = false }
    }
}
