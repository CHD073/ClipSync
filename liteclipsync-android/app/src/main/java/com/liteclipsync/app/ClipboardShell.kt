package com.liteclipsync.app

object ClipboardShell {
    fun available(): Boolean {
        return ShizukuCompat.ping() && ShizukuCompat.hasPermission()
    }
    fun pingBinder() = ShizukuCompat.ping()
    fun hasPermission() = ShizukuCompat.hasPermission()
}
