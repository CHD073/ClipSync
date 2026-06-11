package com.clipsync.app

import android.app.*
import android.content.Intent
import android.os.Build
import android.os.IBinder

class SyncService : Service() {

    override fun onCreate() {
        super.onCreate()
        createNotificationChannel()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == "STOP") {
            stopSelf()
            return START_NOT_STICKY
        }
        val app = application as ClipSyncApp
        app.sm.start()
        startForeground(1, buildNotification())
        app.config.serviceRunning = true
        return START_STICKY
    }

    override fun onDestroy() {
        val app = application as ClipSyncApp
        app.sm.stop()
        app.config.serviceRunning = false
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private fun buildNotification(): Notification {
        val pi = PendingIntent.getActivity(
            this, 0, Intent(this, MainActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
        )
        val stopIntent = Intent(this, SyncService::class.java).apply { action = "STOP" }
        val stopPi = PendingIntent.getService(this, 0, stopIntent, PendingIntent.FLAG_IMMUTABLE)
        return Notification.Builder(this, CHANNEL_ID)
            .setContentTitle("ClipSync")
            .setContentText("Syncing clipboard...")
            .setSmallIcon(android.R.drawable.ic_menu_share)
            .setContentIntent(pi)
            .addAction(android.R.drawable.ic_media_pause, "Stop", stopPi)
            .setOngoing(true)
            .build()
    }

    private fun createNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val nm = getSystemService(NotificationManager::class.java)
            nm.createNotificationChannel(
                NotificationChannel(CHANNEL_ID, "Sync", NotificationManager.IMPORTANCE_LOW).apply {
                    description = "ClipSync background service"
                }
            )
        }
    }

    companion object {
        const val CHANNEL_ID = "clipsync_sync"
    }
}
