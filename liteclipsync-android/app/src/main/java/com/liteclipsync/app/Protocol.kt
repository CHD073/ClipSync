package com.liteclipsync.app

import com.google.gson.Gson
import com.google.gson.annotations.SerializedName
import com.google.gson.reflect.TypeToken

// ── DTO ──
data class ProfileDto(
    @SerializedName("type") val contentType: String = "Text",
    val hash: String = "",
    val text: String = "",
    @SerializedName("has_data") val hasData: Boolean = false,
    @SerializedName("data_name") val dataName: String = "",
    val size: Long = 0,
)

// ── WS 消息 ──
sealed class ServerMsg {
    data class AuthOk(val deviceId: String) : ServerMsg()
    data class AuthError(val reason: String) : ServerMsg()
    data class ClipBroadcast(val payload: ProfileDto, val sourceDeviceId: String, val sourceDeviceName: String) : ServerMsg()
    data class Backlog(val entries: List<ProfileDto>) : ServerMsg()
    data class LatestProfile(val payload: ProfileDto, val sourceDeviceId: String, val createdAt: String) : ServerMsg()
}

// ── JSON 解析 ──
val gson = Gson()
private val mapType = object : TypeToken<Map<String, Any?>>() {}.type

fun parseServerMsg(json: String): ServerMsg? = try {
    val m: Map<String, Any?> = gson.fromJson(json, mapType)
    val p = gson.toJson(m["payload"])
    when (m["type"]) {
        "AuthOk" -> ServerMsg.AuthOk(m["device_id"] as? String ?: "")
        "AuthError" -> ServerMsg.AuthError(m["reason"] as? String ?: "")
        "ClipBroadcast" -> ServerMsg.ClipBroadcast(
            payload = gson.fromJson(p, ProfileDto::class.java),
            sourceDeviceId = m["source_device_id"] as? String ?: "",
            sourceDeviceName = m["source_device_name"] as? String ?: "",
        )
        "Backlog" -> ServerMsg.Backlog(gson.fromJson(p ?: gson.toJson(m["entries"]), object : TypeToken<List<ProfileDto>>() {}.type))
        "LatestProfile" -> {
            val pl: ProfileDto = gson.fromJson(p, ProfileDto::class.java)
            ServerMsg.LatestProfile(pl, m["source_device_id"] as? String ?: "", m["created_at"] as? String ?: "")
        }
        else -> null
    }
} catch (_: Exception) { null }

fun buildAuth(token: String, deviceId: String, name: String) = gson.toJson(mapOf(
    "type" to "Auth", "token" to token, "device_id" to deviceId, "name" to name,
))

fun buildLiteClipSync(payload: ProfileDto, deviceId: String) = gson.toJson(mapOf(
    "type" to "LiteClipSync", "payload" to payload, "device_id" to deviceId,
))
