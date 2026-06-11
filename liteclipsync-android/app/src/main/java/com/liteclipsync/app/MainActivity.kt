package com.liteclipsync.app

import android.Manifest
import android.app.ActivityManager
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.*
import rikka.shizuku.Shizuku

class MainActivity : ComponentActivity() {
    private val sm get() = (application as LiteClipSyncApp).sm
    private val cfg get() = (application as LiteClipSyncApp).config

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            if (checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
                requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 0)
            }
        }
        setContent { MaterialTheme { MainScreen() } }
    }

    @Composable
    private fun MainScreen() {
        var serverUrl by remember { mutableStateOf(cfg.serverUrl) }
        var token by remember { mutableStateOf(cfg.token) }
        var deviceName by remember { mutableStateOf(cfg.deviceName) }
        var connected by remember { mutableStateOf(false) }
        var running by remember { mutableStateOf(false) }
        var showServer by remember { mutableStateOf(false) }
        var autoSync by remember { mutableStateOf(cfg.autoSync) }
        var shizukuOk by remember { mutableStateOf(false) }
        var shizukuRunning by remember { mutableStateOf(false) }
        val logs = remember { mutableStateListOf<String>() }
        val scope = rememberCoroutineScope()
        var progress by remember { mutableStateOf(-1f) }
        var progressLabel by remember { mutableStateOf("") }

        fun serviceAlive(): Boolean {
            val am = getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
            return am.getRunningServices(Int.MAX_VALUE).any { it.service.className == SyncService::class.java.name }
        }

        fun log(msg: String) { logs.add(0, "${java.text.SimpleDateFormat("HH:mm:ss", java.util.Locale.getDefault()).format(java.util.Date())} $msg") }

        LaunchedEffect(Unit) {
            while (true) {
                connected = cfg.connected
                running = serviceAlive()
                shizukuRunning = try { Shizuku.pingBinder() } catch (_: Exception) { false }
                shizukuOk = shizukuRunning && (try { Shizuku.checkSelfPermission() == 0 } catch (_: Exception) { false })
                autoSync = cfg.autoSync
                delay(500)
            }
        }

        val uploadPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri: Uri? ->
            if (uri == null) return@rememberLauncherForActivityResult
            scope.launch(Dispatchers.IO) {
                val cr = contentResolver
                var name = "file"
                var size = -1L
                cr.query(uri, null, null, null, null)?.use { c ->
                    if (c.moveToFirst()) {
                        val ni = c.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                        val si = c.getColumnIndex(OpenableColumns.SIZE)
                        if (ni >= 0) name = c.getString(ni)
                        if (si >= 0) size = c.getLong(si)
                    }
                }
                progress = 0f; progressLabel = "Uploading $name..."
                val result = sm.uploadFromUri(uri, cr, name, size) { done, total -> progress = if (total > 0) done.toFloat() / total.toFloat() else 0f }
                progress = -1f
                logs.add(0, result)
            }
        }

        var pendingFile: java.io.File? = null
        var pendingName = ""
        val savePicker = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/octet-stream")) { uri: Uri? ->
            if (uri == null) return@rememberLauncherForActivityResult
            val src = pendingFile ?: return@rememberLauncherForActivityResult
            scope.launch(Dispatchers.IO) {
                src.inputStream().use { i -> contentResolver.openOutputStream(uri)?.use { o -> i.copyTo(o) } }
                logs.add(0, T.saved(pendingName)); pendingFile = null
            }
        }

        Surface(modifier = Modifier.fillMaxSize()) {
            Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
                Column(modifier = Modifier.weight(1f).verticalScroll(rememberScrollState())) {
                    Text("LiteClipSync", style = MaterialTheme.typography.headlineMedium)
                    Spacer(Modifier.height(8.dp))

                    Card(Modifier.fillMaxWidth(), colors = CardDefaults.cardColors(containerColor = when {
                        shizukuOk -> Color(0xFFE8F5E9)
                        shizukuRunning -> Color(0xFFFFF3E0)
                        else -> Color(0xFFFFEBEE)
                    })) {
                        Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text(if (shizukuOk) "\u2705 Shizuku" else if (shizukuRunning) "\u26A0 Shizuku" else "\u274C Shizuku")
                            Spacer(Modifier.width(8.dp))
                            Text(when { shizukuOk -> T.ready(); shizukuRunning -> T.notAuthorized(); else -> T.notRunning() },
                                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            Spacer(Modifier.weight(1f))
                            if (shizukuRunning && !shizukuOk) {
                                Button(onClick = { Shizuku.requestPermission(0) }, contentPadding = PaddingValues(8.dp, 4.dp)) { Text(T.grant()) }
                            }
                        }
                    }
                    Spacer(Modifier.height(8.dp))

                    Row { Text(T.server(), color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.labelLarge); Spacer(Modifier.width(4.dp)); TextButton(onClick = { showServer = !showServer }, contentPadding = PaddingValues(4.dp)) { Text(if (showServer) "\u25B2" else "\u25BC", style = MaterialTheme.typography.bodySmall) } }
                    if (showServer) {
                        OutlinedTextField(serverUrl, { serverUrl = it }, label = { Text(T.serverUrl()) }, singleLine = true, modifier = Modifier.fillMaxWidth())
                        OutlinedTextField(token, { token = it }, label = { Text(T.token()) }, singleLine = true, modifier = Modifier.fillMaxWidth())
                        Button(onClick = { cfg.serverUrl = serverUrl; cfg.token = token }, modifier = Modifier.fillMaxWidth()) { Text(T.save()) }
                        Spacer(Modifier.height(4.dp))
                    }
                    OutlinedTextField(deviceName, { deviceName = it; cfg.deviceName = it }, label = { Text(T.device()) }, singleLine = true, modifier = Modifier.fillMaxWidth())
                    Spacer(Modifier.height(8.dp))

                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(if (connected) "\uD83D\uDFE2 " + T.connected() else "\uD83D\uDD34 " + T.disconnected(), style = MaterialTheme.typography.labelLarge)
                    }
                    Spacer(Modifier.height(4.dp))

                    Button(onClick = {
                        if (running) { stopService(Intent(this@MainActivity, SyncService::class.java)); running = false; logs.add(0, T.stopped()) }
                        else { startService(Intent(this@MainActivity, SyncService::class.java)); running = true; logs.add(0, T.started()) }
                    }, modifier = Modifier.fillMaxWidth(), colors = if (running) ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.error) else ButtonDefaults.buttonColors()) {
                        Text(if (running) "\u25A0 " + T.stop() else "\u25B6 " + T.start())
                    }
                    Spacer(Modifier.height(4.dp))

                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(T.autoSync(), modifier = Modifier.weight(1f))
                        Switch(checked = autoSync, onCheckedChange = { sm.setAutoSync(it); autoSync = it })
                    }
                    Spacer(Modifier.height(8.dp))

                    Text(T.actions(), color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.labelLarge)
                    Spacer(Modifier.height(4.dp))
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(onClick = {
                            scope.launch(Dispatchers.IO) {
                                val cm = getSystemService(Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
                                val txt = cm.primaryClip?.getItemAt(0)?.text?.toString()
                                if (txt != null) logs.add(0, sm.uploadText(txt)) else logs.add(0, T.noText())
                            }
                        }, modifier = Modifier.weight(1f)) { Text(T.upload()) }
                        Button(onClick = {
                            scope.launch(Dispatchers.IO) {
                                logs.add(0, "fetching...")
                                val p = sm.fetchLatest() ?: return@launch
                                if (p.hash.isEmpty()) { logs.add(0, "empty"); return@launch }
                                if (p.contentType == "Text") {
                                    sm.applyRemote(p)
                                    logs.add(0, "text set (${p.text.length} chars)")
                                } else if (p.dataName.isNotEmpty()) {
                                    progress = 0f; progressLabel = "Downloading..."
                                    val tmp = sm.downloadToTempFile(p.dataName) { done, total -> progress = if (total > 0) done.toFloat() / total.toFloat() else -1f }
                                    progress = -1f
                                    if (tmp == null) { logs.add(0, "download failed"); return@launch }
                                    pendingFile = tmp; pendingName = p.dataName
                                    withContext(Dispatchers.Main) { savePicker.launch(p.dataName) }
                                } else logs.add(0, "type=${p.contentType}")
                            }
                        }, modifier = Modifier.weight(1f)) { Text(T.download()) }
                    }
                    Button(onClick = { uploadPicker.launch(arrayOf("*/*")) }, modifier = Modifier.fillMaxWidth()) { Text(T.uploadFile()) }
                    Spacer(Modifier.height(8.dp))

                    var lt by remember { mutableStateOf("") }; var lf by remember { mutableStateOf("") }
                    LaunchedEffect(Unit) { while (true) { lt = cfg.lastSyncTime; lf = cfg.lastSyncFrom; delay(1000) } }
                    if (lt.isNotEmpty()) {
                        Text(T.lastSync(), color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.labelLarge)
                        Card(Modifier.fillMaxWidth()) { Column(Modifier.padding(12.dp)) {
                            Text(lt); if (lf.isNotEmpty()) Text(T.from() + " $lf", color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodySmall)
                        } }
                    }
                }

                if (progress >= 0f) {
                    LinearProgressIndicator(progress = { progress.coerceIn(0f, 1f) }, modifier = Modifier.fillMaxWidth())
                    if (progressLabel.isNotEmpty()) Text(progressLabel, style = MaterialTheme.typography.bodySmall)
                }

                HorizontalDivider()
                Text(T.log(), style = MaterialTheme.typography.labelSmall)
                Box(Modifier.height(120.dp).fillMaxWidth().padding(4.dp)) {
                    val scroll = rememberScrollState()
                    Text(logs.joinToString("\n"), style = MaterialTheme.typography.bodySmall, modifier = Modifier.fillMaxWidth().verticalScroll(scroll))
                }
            }
        }
    }
}

