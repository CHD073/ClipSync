package com.liteclipsync.app

import android.content.ContentResolver
import android.net.Uri
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okio.BufferedSink
import java.io.File
import java.io.FileOutputStream
import java.util.concurrent.TimeUnit

class HttpApi(private val cfg: Config) {
    private val client = OkHttpClient.Builder()
        .connectTimeout(30, TimeUnit.SECONDS)
        .readTimeout(180, TimeUnit.SECONDS)
        .writeTimeout(180, TimeUnit.SECONDS)
        .build()

    // Small files: upload byte array (existing, used for text)
    suspend fun upload(name: String, data: ByteArray): Result<Unit> = withContext(Dispatchers.IO) {
        try {
            val r = Request.Builder().url("${cfg.apiBase}/file/$name")
                .put(data.toRequestBody("application/octet-stream".toMediaType()))
                .header("Authorization", cfg.authHeader).build()
            val resp = client.newCall(r).execute()
            if (resp.isSuccessful) Result.success(Unit)
            else Result.failure(Exception("HTTP ${resp.code}"))
        } catch (e: Exception) { Result.failure(e) }
    }

    // Large files: stream from ContentResolver URI with progress
    suspend fun uploadStream(
        name: String, uri: Uri, cr: ContentResolver, totalSize: Long,
        onProgress: (Long, Long) -> Unit,
    ): Result<Unit> = withContext(Dispatchers.IO) {
        try {
            val input = cr.openInputStream(uri) ?: return@withContext Result.failure(Exception("cannot open URI"))
            val body = StreamBody(input, totalSize, onProgress)
            val r = Request.Builder().url("${cfg.apiBase}/file/$name")
                .put(body).header("Authorization", cfg.authHeader).build()
            val resp = client.newCall(r).execute()
            if (resp.isSuccessful) Result.success(Unit)
            else Result.failure(Exception("HTTP ${resp.code}"))
        } catch (e: Exception) { Result.failure(e) }
    }

    // Small files: download byte array
    suspend fun download(name: String): Result<ByteArray> = withContext(Dispatchers.IO) {
        try {
            val r = Request.Builder().url("${cfg.apiBase}/file/$name").get()
                .header("Authorization", cfg.authHeader).build()
            val resp = client.newCall(r).execute()
            if (resp.isSuccessful) Result.success(resp.body?.bytes() ?: ByteArray(0))
            else Result.failure(Exception("HTTP ${resp.code}"))
        } catch (e: Exception) { Result.failure(e) }
    }

    // Large files: stream to file with progress
    suspend fun downloadToFile(
        name: String, outputFile: File,
        onProgress: (Long, Long) -> Unit,
    ): Result<Unit> = withContext(Dispatchers.IO) {
        try {
            val r = Request.Builder().url("${cfg.apiBase}/file/$name").get()
                .header("Authorization", cfg.authHeader).build()
            val resp = client.newCall(r).execute()
            if (!resp.isSuccessful) return@withContext Result.failure(Exception("HTTP ${resp.code}"))
            val total = resp.body?.contentLength() ?: -1L
            val input = resp.body?.byteStream() ?: return@withContext Result.failure(Exception("no body"))
            FileOutputStream(outputFile).use { out ->
                val buf = ByteArray(8192)
                var read = 0L
                while (true) {
                    val n = input.read(buf)
                    if (n < 0) break
                    out.write(buf, 0, n)
                    read += n
                    onProgress(read, total)
                }
            }
            Result.success(Unit)
        } catch (e: Exception) { Result.failure(e) }
    }
}

// Streaming upload: reads from InputStream in chunks
private class StreamBody(
    private val input: java.io.InputStream,
    private val total: Long,
    private val onProgress: (Long, Long) -> Unit,
) : okhttp3.RequestBody() {
    override fun contentType() = "application/octet-stream".toMediaType()
    override fun contentLength() = if (total > 0) total else -1L
    override fun writeTo(sink: BufferedSink) {
        val buf = ByteArray(8192)
        var written = 0L
        input.use { src ->
            while (true) {
                val n = src.read(buf)
                if (n < 0) break
                sink.write(buf, 0, n)
                written += n
                onProgress(written, total)
            }
        }
    }
}
