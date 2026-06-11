package com.liteclipsync.app

import java.util.Locale

object T {
    val zh: Boolean get() = Locale.getDefault().language == "zh"

    fun app_name() = if (zh) "剪贴板同步" else "LiteClipSync"
    fun shizuku() = if (zh) "Shizuku" else "Shizuku"
    fun ready() = if (zh) "就绪" else "Ready"
    fun notAuthorized() = if (zh) "未授权" else "Not authorized"
    fun notRunning() = if (zh) "未运行" else "Not running"
    fun grant() = if (zh) "授权" else "Grant"
    fun server() = if (zh) "服务器" else "Server"
    fun serverUrl() = if (zh) "服务器地址" else "Server URL"
    fun token() = if (zh) "令牌" else "Token"
    fun save() = if (zh) "保存" else "Save"
    fun device() = if (zh) "设备名" else "Device name"
    fun connected() = if (zh) "已连接" else "Connected"
    fun disconnected() = if (zh) "已断开" else "Disconnected"
    fun start() = if (zh) "启动" else "Start"
    fun stop() = if (zh) "停止" else "Stop"
    fun autoSync() = if (zh) "自动同步" else "Auto Sync"
    fun actions() = if (zh) "操作" else "Actions"
    fun upload() = if (zh) "上传" else "Upload"
    fun download() = if (zh) "下载" else "Download"
    fun uploadFile() = if (zh) "上传文件" else "Upload File"
    fun lastSync() = if (zh) "上次同步" else "Last Sync"
    fun from() = if (zh) "来自" else "From"
    fun log() = if (zh) "日志" else "Log"
    fun noText() = if (zh) "剪贴板无文字" else "no text in clipboard"
    fun empty() = if (zh) "空" else "empty"
    fun textSet(n: Int) = if (zh) "文字已设置 ($n 字符)" else "text set ($n chars)"
    fun fetching() = if (zh) "获取中..." else "fetching..."
    fun downloading() = if (zh) "下载中..." else "Downloading..."
    fun uploadFailed() = if (zh) "上传失败" else "upload failed"
    fun downloadFailed() = if (zh) "下载失败" else "download failed"
    fun saved(name: String) = if (zh) "已保存: $name" else "saved: $name"
    fun stopped() = if (zh) "已停止" else "stopped"
    fun started() = if (zh) "已启动" else "started"
    fun uploading(name: String) = if (zh) "上传中: $name" else "Uploading $name"

    // Notification
    fun syncService() = if (zh) "剪贴板同步服务" else "LiteClipSync"
    fun syncing() = if (zh) "正在同步剪贴板..." else "Syncing clipboard..."
    fun syncChannel() = if (zh) "同步" else "Sync"
    fun syncChannelDesc() = if (zh) "后台同步通知" else "Background sync notification"
}
