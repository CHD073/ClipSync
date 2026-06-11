package com.clipsync.app

import android.content.ClipData
import android.os.IBinder
import android.os.RemoteException
import android.util.Log

class ClipSyncUserService : IClipSyncService.Stub() {

    companion object {
        private const val TAG = "ClipSync"
        private const val PACKAGE_NAME = "com.android.shell"
        private var clipboardService: Any? = null

        init {
            if (android.os.Process.myUid() == 0) {
                try {
                    android.system.Os.setgid(2000)
                    android.system.Os.setuid(2000)
                    Log.i(TAG, "UserService UID switched root->shell")
                } catch (e: Exception) {
                    Log.e(TAG, "UserService UID switch failed: ${e.message}")
                }
            }
        }

        @Synchronized
        private fun getClipboard(): Any? {
            if (clipboardService != null) return clipboardService
            return try {
                val sm = Class.forName("android.os.ServiceManager")
                val binder = sm.getMethod("getService", String::class.java).invoke(null, "clipboard") as? IBinder
                if (binder == null) return null
                val stub = Class.forName("android.content.IClipboard\$Stub")
                clipboardService = stub.getMethod("asInterface", IBinder::class.java).invoke(null, binder)
                Log.d(TAG, "UserService got clipboard: ${clipboardService?.javaClass?.name}")
                clipboardService
            } catch (e: Exception) {
                Log.e(TAG, "UserService clipboard init: ${e.message}")
                null
            }
        }

        private fun invokeIface(obj: Any, methodName: String): Any? {
            val methods = obj.javaClass.methods
                .filter { it.name == methodName }
                .sortedByDescending { it.parameterCount }
            for (m in methods) {
                val args = arrayOfNulls<Any?>(m.parameterCount)
                for (i in m.parameterTypes.indices) {
                    when {
                        m.parameterTypes[i] == String::class.java -> args[i] = PACKAGE_NAME
                        m.parameterTypes[i] == Int::class.javaPrimitiveType || m.parameterTypes[i] == Int::class.java -> args[i] = 0
                        m.parameterTypes[i] == Long::class.javaPrimitiveType || m.parameterTypes[i] == Long::class.java -> args[i] = 0L
                        m.parameterTypes[i] == Boolean::class.javaPrimitiveType || m.parameterTypes[i] == Boolean::class.java -> args[i] = false
                        else -> args[i] = null
                    }
                }
                try {
                    return m.invoke(obj, *args)
                } catch (e: Exception) {
                    if (e is java.lang.reflect.InvocationTargetException) continue
                }
            }
            return null
        }
    }

    override fun getPrimaryClipText(): String {
        return try {
            val cb = getClipboard() ?: return ""
            val clip = invokeIface(cb, "getPrimaryClip") as? ClipData
            if (clip == null || clip.itemCount == 0) return ""
            val text = clip.getItemAt(0).text?.toString() ?: ""
            if (text.isNotEmpty()) Log.d(TAG, "UserService read: ${text.length} chars")
            text
        } catch (e: Exception) {
            Log.e(TAG, "getText err: ${e.message}")
            ""
        }
    }

    override fun setPrimaryClipText(text: String) {
        try {
            val cb = getClipboard() ?: return
            val clip = ClipData.newPlainText("clean", text)
            invokeIface(cb, "setPrimaryClip")
        } catch (e: Exception) {
            Log.e(TAG, "setText err: ${e.message}")
        }
    }

    override fun onTransact(code: Int, data: android.os.Parcel, reply: android.os.Parcel?, flags: Int): Boolean {
        return try {
            super.onTransact(code, data, reply, flags)
        } catch (e: RemoteException) {
            Log.e(TAG, "onTransact: ${e.message}")
            false
        }
    }
}
