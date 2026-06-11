package com.clipsync.app

import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.net.Uri
import android.os.Bundle
import android.os.IBinder
import android.util.Log
import moe.shizuku.api.BinderContainer
import rikka.shizuku.Shizuku

class ShizukuApiProvider : ContentProvider() {

    @Volatile private var done = false

    override fun onCreate(): Boolean {
        Log.d("ClipSync", "Shizuku provider ready")
        return true
    }

    override fun call(method: String, arg: String?, extras: Bundle?): Bundle? {
        if (method != "sendBinder" || done) return null
        if (extras == null || context == null) return null
        try {
            extras.classLoader = context!!.classLoader
            val binder = extractBinder(extras)
            if (binder != null) {
                done = true
                Log.d("ClipSync", "Shizuku binder received, ping=${ShizukuCompat.ping()}")
                Shizuku.onBinderReceived(binder, context!!.packageName)
            }
        } catch (e: Throwable) {
            Log.e("ClipSync", "Shizuku binder failed: ${e.message}")
            done = false
        }
        return null
    }

    private fun extractBinder(b: Bundle): IBinder? {
        for (key in b.keySet()) {
            val v = try { b[key] } catch (_: Exception) { continue }
            when (v) {
                is IBinder -> return v
                is BinderContainer -> return v.binder
            }
        }
        return null
    }

    override fun query(uri: Uri, projection: Array<out String>?, selection: String?,
                       selectionArgs: Array<out String>?, sortOrder: String?): Cursor? = null
    override fun getType(uri: Uri): String? = null
    override fun insert(uri: Uri, values: ContentValues?): Uri? = null
    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0
    override fun update(uri: Uri, values: ContentValues?, selection: String?,
                        selectionArgs: Array<out String>?): Int = 0
}
