package com.clipsync.app

import android.app.Application
import android.util.Log
import rikka.shizuku.Shizuku

class ClipSyncApp : Application() {
    val config: Config by lazy { Config(this) }
    val sm: SyncManager by lazy { SyncManager(this) }

    private val binderListener = Shizuku.OnBinderReceivedListener {
        Log.d("ClipSync", "Shizuku ready, ping=${Shizuku.pingBinder()}")
    }

    override fun onCreate() {
        super.onCreate()
        Shizuku.addBinderReceivedListener(binderListener)
    }
}
