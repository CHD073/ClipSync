package com.clipsync.app

import android.util.Log
import rikka.shizuku.Shizuku
import java.lang.reflect.Method

class ShizukuCompat {
    companion object {
        private var newProcessMethod: Method? = null
        init {
            try {
                newProcessMethod = Shizuku::class.java.getDeclaredMethod("newProcess",
                    Array<String>::class.java, Array<String>::class.java, String::class.java)
                newProcessMethod?.isAccessible = true
            } catch (_: Exception) {}
        }

        @JvmStatic fun ping() = try { Shizuku.pingBinder() } catch (_: Exception) { false }
        @JvmStatic fun hasPermission() = try { Shizuku.checkSelfPermission() == 0 } catch (_: Exception) { false }

        @Suppress("DEPRECATION")
        @JvmStatic fun newProcess(cmd: Array<String>): Process? {
            try {
                return newProcessMethod?.invoke(null, cmd, null, null) as? Process
            } catch (e: Exception) {
                Log.e("ClipSync", "newProcess err: ${e.message}")
                return null
            }
        }
    }
}
