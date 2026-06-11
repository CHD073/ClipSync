package com.liteclipsync.app

import android.content.Context
import android.content.SharedPreferences
import java.util.UUID

class Config(context: Context) {
    private val p: SharedPreferences = context.getSharedPreferences("liteclipsync", Context.MODE_PRIVATE)

    var serverUrl: String
        get() = p.getString("server_url", "http://192.168.245.134:8765") ?: "http://192.168.245.134:8765"
        set(v) = p.edit().putString("server_url", v).apply()

    var token: String
        get() = p.getString("token", "liteclipsync") ?: "liteclipsync"
        set(v) = p.edit().putString("token", v).apply()

    val deviceId: String
        get() = p.getString("device_id", null) ?: UUID.randomUUID().toString().also {
            p.edit().putString("device_id", it).apply()
        }

    var deviceName: String
        get() = p.getString("device_name", null) ?: "${android.os.Build.MANUFACTURER} ${android.os.Build.MODEL}".also {
            p.edit().putString("device_name", it).apply()
        }
        set(v) = p.edit().putString("device_name", v).apply()

    var autoSyncMaxBytes: Long
        get() = p.getLong("auto_sync_max_bytes", 10_485_760)
        set(v) = p.edit().putLong("auto_sync_max_bytes", v).apply()

    var serviceRunning: Boolean
        get() = p.getBoolean("service_running", false)
        set(v) = p.edit().putBoolean("service_running", v).apply()

    var autoSync: Boolean
        get() = p.getBoolean("auto_sync", true)
        set(v) = p.edit().putBoolean("auto_sync", v).apply()

    var connected: Boolean
        get() = p.getBoolean("connected", false)
        set(v) = p.edit().putBoolean("connected", v).apply()

    var lastSyncTime: String
        get() = p.getString("last_sync_time", "") ?: ""
        set(v) = p.edit().putString("last_sync_time", v).apply()

    var lastSyncFrom: String
        get() = p.getString("last_sync_from", "") ?: ""
        set(v) = p.edit().putString("last_sync_from", v).apply()

    val apiBase: String get() = serverUrl.trimEnd('/')
    val wsUrl: String get() = serverUrl.replace("http://", "ws://").replace("https://", "wss://") + "/ws"
    val authHeader: String get() = "Basic ${android.util.Base64.encodeToString("$token:".toByteArray(), android.util.Base64.NO_WRAP)}"
}
