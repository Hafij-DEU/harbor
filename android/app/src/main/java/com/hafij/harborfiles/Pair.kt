package com.hafij.harborfiles

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import org.json.JSONObject
import java.io.IOException
import java.util.concurrent.TimeUnit

/** A Harbor desktop pairing code: harborpair://ip:port/TOKEN ("Scan with the phone"). */
data class PairCode(val host: String, val port: Int, val token: String)

fun parsePairQr(text: String): PairCode? {
    val m = Regex("""^harborpair://(\[[0-9a-fA-F:.]+]|[A-Za-z0-9.-]+):(\d{1,5})/([A-Za-z0-9_-]{8,128})/?$""")
        .find(text.trim()) ?: return null
    val port = m.groupValues[2].toIntOrNull()?.takeIf { it in 1..65535 } ?: return null
    return PairCode(m.groupValues[1], port, m.groupValues[3])
}

/**
 * Waits for this phone's share (start it first) and sends its ftp:// code with the
 * PIN to the PC that showed [code]. The PC opens the phone. Returns null on success,
 * else a message. The PIN and token are never logged.
 */
suspend fun pairWithPc(code: PairCode): String? = withContext(Dispatchers.IO) {
    var info: ServerInfo? = null
    for (i in 0 until 60) {
        info = ServerService.state.value
        if (info != null || ServerService.error.value != null) break
        delay(200)
    }
    val share = info ?: return@withContext ServerService.error.value ?: "This phone's share did not start"
    val body = JSONObject().put("token", code.token).put("ftp", share.qrText).toString()
        .toRequestBody("application/json".toMediaType())
    val client = OkHttpClient.Builder().callTimeout(10, TimeUnit.SECONDS).build()
    try {
        client.newCall(Request.Builder().url("http://${code.host}:${code.port}/pair").post(body).build())
            .execute().use { r ->
                when (r.code) {
                    200 -> null
                    403 -> "The PC refused the code. Show a new code on the PC and scan again."
                    409 -> "That code was already used. Show a new code on the PC."
                    else -> "The PC answered ${r.code}"
                }
            }
    } catch (e: IOException) {
        "Can't reach the PC. Are both on the same Wi-Fi? (${e.message ?: "no answer"})"
    }
}
