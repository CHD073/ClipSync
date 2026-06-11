package com.clipsync.app

import android.app.Application
import android.content.Intent
import android.os.Build
import android.util.Log
import rikka.shizuku.Shizuku

class ClipSyncApp : Application() {
    val config: Config by lazy { Config(this) }
    val sm: SyncManager by lazy { SyncManager(this) }

    private val binderReceivedListener = Shizuku.OnBinderReceivedListener {
        Log.d("ClipSync", "Shizuku binder received, perm=${Shizuku.checkSelfPermission()}")
        if (Shizuku.checkSelfPermission() == 0) ShizukuShell.bindOnce()
        else Shizuku.requestPermission(0)
    }

    private val permissionResultListener = Shizuku.OnRequestPermissionResultListener { _, result ->
        Log.d("ClipSync", "Shizuku permission result=$result")
        if (result == 0) ShizukuShell.bindOnce()
    }

    override fun onCreate() {
        super.onCreate()
        Shizuku.addBinderReceivedListenerSticky(binderReceivedListener)
        Shizuku.addRequestPermissionResultListener(permissionResultListener)
        val intent = Intent(this, SyncService::class.java)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) startForegroundService(intent)
        else startService(intent)
    }
}

