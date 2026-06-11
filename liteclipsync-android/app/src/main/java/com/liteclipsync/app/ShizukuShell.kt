package com.liteclipsync.app

import android.content.ComponentName
import android.content.ServiceConnection
import android.os.IBinder
import android.util.Log
import rikka.shizuku.Shizuku

object ShizukuShell {
    private var svc: ILiteClipSyncService? = null
    private var bound = false

    private val conn = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
            svc = binder?.let { ILiteClipSyncService.Stub.asInterface(it) }
            bound = true
            Log.d("LiteClipSync", "ShizukuShell: bound")
        }
        override fun onServiceDisconnected(name: ComponentName?) {
            svc = null; bound = false
            Log.d("LiteClipSync", "ShizukuShell: disconnected")
        }
    }

    fun bindOnce() {
        if (bound || svc != null) return
        try {
            val args = Shizuku.UserServiceArgs(
                ComponentName("com.liteclipsync.app", LiteClipSyncUserService::class.java.name)
            ).daemon(false).processNameSuffix("shizuku_clip").debuggable(true).version(1)
            Shizuku.bindUserService(args, conn)
            Log.d("LiteClipSync", "ShizukuShell: bindUserService called")
        } catch (e: Exception) {
            Log.e("LiteClipSync", "ShizukuShell bind err: ${e.message}")
        }
    }

    fun getText(): String? {
        val s = svc ?: return null
        return try {
            val t = s.getPrimaryClipText()
            if (t.isNotEmpty()) Log.d("LiteClipSync", "ShizukuShell read: ${t.length} chars")
            t.takeIf { it.isNotEmpty() }
        } catch (e: Exception) {
            Log.e("LiteClipSync", "ShizukuShell read err: ${e.message}")
            null
        }
    }

    fun available() = bound && svc != null
}
