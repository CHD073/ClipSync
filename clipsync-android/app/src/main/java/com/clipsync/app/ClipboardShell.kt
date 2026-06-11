package com.clipsync.app

import android.util.Log

object ClipboardShell {
    fun getText(): String? {
        if (!available()) return null
        try {
            val p = ShizukuCompat.newProcess(arrayOf("service", "call", "clipboard", "1", "s16", "com.clipsync.app", "i32", "0"))
                ?: return null
            val text = extractClipText(p.inputStream.readBytes())
            p.waitFor()
            return text
        } catch (e: Exception) {
            Log.e("ClipSync", "shizuku getText: ${e.message}")
            return null
        }
    }

    fun setText(text: String): Boolean {
        if (!available()) return false
        try {
            val p = ShizukuCompat.newProcess(arrayOf("cmd", "clipboard", "set-text", text)) ?: return false
            p.waitFor()
            return p.exitValue() == 0
        } catch (_: Exception) { return false }
    }

    fun available(): Boolean {
        val ping = ShizukuCompat.ping()
        val perm = if (ping) ShizukuCompat.hasPermission() else false
        return ping && perm
    }

    fun pingBinder() = ShizukuCompat.ping()
    fun hasPermission() = ShizukuCompat.hasPermission()

    // Parse text from `service call clipboard` Parcel output
    private fun extractClipText(data: ByteArray): String? {
        val str = String(data)
        val skip = setOf("Result", "Parcel", "No items", "Stub", "Proxy", "IClipboard", "android", "server", "java", "ClipboardService")
        val sb = StringBuilder()
        var best = ""
        for (ch in str) {
            if (ch in ' '..'~' || ch.code > 127) sb.append(ch)
            else { if (sb.length > best.length) best = sb.toString(); sb.clear() }
        }
        if (sb.length > best.length) best = sb.toString()
        return if (best.length > 2 && best !in skip) best else null
    }
}
