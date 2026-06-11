package com.clipsync.app

import android.Manifest
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
    private val sm get() = (application as ClipSyncApp).sm
    private val cfg get() = (application as ClipSyncApp).config

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
        var running by remember { mutableStateOf(cfg.serviceRunning) }
        var autoSync by remember { mutableStateOf(cfg.autoSync) }
        var showServer by remember { mutableStateOf(false) }
        var shizukuOk by remember { mutableStateOf(false) }
        var shizukuRunning by remember { mutableStateOf(false) }
        val logs = remember { mutableStateListOf<String>() }
        val scope = rememberCoroutineScope()
        var progress by remember { mutableStateOf(-1f) }
        var progressLabel by remember { mutableStateOf("") }

        LaunchedEffect(Unit) {
            while (true) {
                connected = cfg.connected
                running = cfg.serviceRunning
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
                logs.add(0, "saved: $pendingName"); pendingFile = null
            }
        }

        Surface(modifier = Modifier.fillMaxSize()) {
            Column(modifier = Modifier.fillMaxSize().padding(16.dp)) {
                Column(modifier = Modifier.weight(1f).verticalScroll(rememberScrollState())) {
                    Text("ClipSync", style = MaterialTheme.typography.headlineMedium)
                    Spacer(Modifier.height(8.dp))

                    // Shizuku status
                    Card(Modifier.fillMaxWidth(), colors = CardDefaults.cardColors(containerColor = when {
                        shizukuOk -> Color(0xFFE8F5E9)
                        shizukuRunning -> Color(0xFFFFF3E0)
                        else -> Color(0xFFFFEBEE)
                    })) {
                        Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically) {
                            Text(if (shizukuOk) "\u2705 Shizuku" else if (shizukuRunning) "\u26A0 Shizuku" else "\u274C Shizuku")
                            Spacer(Modifier.width(8.dp))
                            Text(when { shizukuOk -> "Ready"; shizukuRunning -> "Not authorized"; else -> "Not running" },
                                style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                            Spacer(Modifier.weight(1f))
                            if (shizukuRunning && !shizukuOk) {
                                Button(onClick = { Shizuku.requestPermission(0) }, contentPadding = PaddingValues(8.dp, 4.dp)) { Text("Grant") }
                            }
                        }
                    }
                    Spacer(Modifier.height(8.dp))

                    // Server config
                    Row { Text("Server", color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.labelLarge); Spacer(Modifier.width(4.dp)); TextButton(onClick = { showServer = !showServer }, contentPadding = PaddingValues(4.dp)) { Text(if (showServer) "\u25B2" else "\u25BC", style = MaterialTheme.typography.bodySmall) } }
                    if (showServer) {
                        OutlinedTextField(serverUrl, { serverUrl = it }, label = { Text("Server URL") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                        OutlinedTextField(token, { token = it }, label = { Text("Token") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                        Button(onClick = { cfg.serverUrl = serverUrl; cfg.token = token }, modifier = Modifier.fillMaxWidth()) { Text("Save") }
                        Spacer(Modifier.height(4.dp))
                    }
                    OutlinedTextField(deviceName, { deviceName = it; cfg.deviceName = it }, label = { Text("Device name") }, singleLine = true, modifier = Modifier.fillMaxWidth())
                    Spacer(Modifier.height(8.dp))

                    // Connection status
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text(if (connected) "\uD83D\uDFE2 Connected" else "\uD83D\uDD34 Disconnected", style = MaterialTheme.typography.labelLarge)
                    }
                    Spacer(Modifier.height(4.dp))

                    // Start / Stop
                    Button(onClick = {
                        if (running) { stopService(Intent(this@MainActivity, SyncService::class.java)); running = false; logs.add(0, "stopped") }
                        else { startService(Intent(this@MainActivity, SyncService::class.java)); running = true; logs.add(0, "started") }
                    }, modifier = Modifier.fillMaxWidth(), colors = if (running) ButtonDefaults.buttonColors(containerColor = MaterialTheme.colorScheme.error) else ButtonDefaults.buttonColors()) {
                        Text(if (running) "\u25A0 Stop" else "\u25B6 Start")
                    }
                    Spacer(Modifier.height(4.dp))

                    // Auto Sync toggle
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Text("Auto Sync", modifier = Modifier.weight(1f))
                        Switch(checked = autoSync, onCheckedChange = { sm.setAutoSync(it); autoSync = it })
                    }
                    Spacer(Modifier.height(8.dp))

                    // Actions
                    Text("Actions", color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.labelLarge)
                    Spacer(Modifier.height(4.dp))
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                        Button(onClick = {
                            scope.launch(Dispatchers.IO) {
                                val cm = getSystemService(Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
                                val txt = cm.primaryClip?.getItemAt(0)?.text?.toString()
                                if (txt != null) logs.add(0, sm.uploadText(txt)) else logs.add(0, "no text in clipboard")
                            }
                        }, modifier = Modifier.weight(1f)) { Text("Upload") }
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
                        }, modifier = Modifier.weight(1f)) { Text("Download") }
                    }
                    Button(onClick = { uploadPicker.launch(arrayOf("*/*")) }, modifier = Modifier.fillMaxWidth()) { Text("Upload File") }
                    Spacer(Modifier.height(8.dp))

                    // Last sync
                    var lt by remember { mutableStateOf("") }; var lf by remember { mutableStateOf("") }
                    LaunchedEffect(Unit) { while (true) { lt = cfg.lastSyncTime; lf = cfg.lastSyncFrom; delay(1000) } }
                    if (lt.isNotEmpty()) {
                        Text("Last Sync", color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.labelLarge)
                        Card(Modifier.fillMaxWidth()) { Column(Modifier.padding(12.dp)) {
                            Text(lt); if (lf.isNotEmpty()) Text("From: $lf", color = MaterialTheme.colorScheme.onSurfaceVariant, style = MaterialTheme.typography.bodySmall)
                        } }
                    }
                }

                if (progress >= 0f) {
                    LinearProgressIndicator(progress = { progress.coerceIn(0f, 1f) }, modifier = Modifier.fillMaxWidth())
                    if (progressLabel.isNotEmpty()) Text(progressLabel, style = MaterialTheme.typography.bodySmall)
                }

                HorizontalDivider()
                Text("Log", style = MaterialTheme.typography.labelSmall)
                Box(Modifier.height(120.dp).fillMaxWidth().padding(4.dp)) {
                    val scroll = rememberScrollState()
                    Text(logs.joinToString("\n"), style = MaterialTheme.typography.bodySmall, modifier = Modifier.fillMaxWidth().verticalScroll(scroll))
                }
            }
        }
    }
}
