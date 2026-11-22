package com.hafij.harborfiles

import okhttp3.OkHttpClient
import okhttp3.Request
import java.io.BufferedInputStream
import java.io.ByteArrayOutputStream
import java.io.InputStream
import java.io.OutputStream
import java.net.InetAddress
import java.net.ServerSocket
import java.net.Socket
import java.net.URLEncoder
import java.security.SecureRandom
import java.util.concurrent.TimeUnit
import kotlin.concurrent.thread

/** Videos at least this big ask "Stream or download?"... */
const val VIDEO_MIN = 5L * 1024 * 1024
/** ...and music at least this big. */
const val AUDIO_MIN = 50L * 1024 * 1024

fun isMedia(mime: String?) = mime != null && (mime.startsWith("video/") || mime.startsWith("audio/"))

fun wantsChoice(mime: String?, size: Long) = when {
    mime == null -> false
    mime.startsWith("video/") -> size >= VIDEO_MIN
    mime.startsWith("audio/") -> size >= AUDIO_MIN
    else -> false
}

/**
 * "Download" copies of server videos and music, deleted as soon as the user
 * comes back from the player (and any left over at start-up). A player that
 * still has the file open (picture-in-picture) keeps playing: Android keeps a
 * deleted file readable until it is closed.
 */
object PlayTemp {
    private val files = HashMap<java.io.File, Long>()

    fun dir(ctx: android.content.Context) = java.io.File(ctx.cacheDir, "play")

    @Synchronized fun add(f: java.io.File) { files[f] = System.currentTimeMillis() }

    /** Called when Harbor is back in front. */
    @Synchronized fun cleanup() {
        val now = System.currentTimeMillis()
        files.entries.removeAll { (f, t) ->
            if (now - t < 2_000) false else { f.parentFile?.deleteRecursively(); true }
        }
    }

    fun cleanAll(ctx: android.content.Context) { dir(ctx).deleteRecursively() }
}

/** Where a server file can be read with HTTP Range requests (the PC's WebDAV share). */
data class StreamSource(val url: String, val headers: Map<String, String>, val size: Long, val mime: String)

/**
 * Lets a video player stream a file from the PC without the PIN.
 *
 * Players get http://127.0.0.1:port/TOKEN/name. Each request, with its Range
 * for seeking, is passed to the PC with the PIN added here, so the player
 * never sees it. Listens on the loopback address only; every file has its own
 * random 256-bit token and anything else is 404. Only GET and HEAD. Nothing
 * is logged (the URL holds the token).
 */
class StreamProxy(private val client: OkHttpClient = defaultClient()) {
    private val items = LinkedHashMap<String, StreamSource>()
    private var server: ServerSocket? = null
    private val rnd = SecureRandom()

    val port: Int get() = server?.localPort ?: -1
    /** Requests being served now, and when the player last read (for [StreamService]). */
    val active = java.util.concurrent.atomic.AtomicInteger()
    @Volatile var lastUse = 0L

    @Synchronized
    fun add(src: StreamSource, name: String): String {
        val b = ByteArray(32).also { rnd.nextBytes(it) }
        val token = java.util.Base64.getUrlEncoder().withoutPadding().encodeToString(b)
        items[token] = src
        while (items.size > MAX_ITEMS) items.remove(items.keys.first())
        // IPv4 127.0.0.1, as in the link: getLoopbackAddress() is ::1 on some phones.
        val ss = server ?: ServerSocket(0, 16, InetAddress.getByAddress(byteArrayOf(127, 0, 0, 1))).also { s ->
            server = s
            thread(name = "stream-accept", isDaemon = true) {
                while (!s.isClosed) {
                    val c = try { s.accept() } catch (_: Exception) { break }
                    thread(name = "stream", isDaemon = true) {
                        active.incrementAndGet(); lastUse = System.currentTimeMillis()
                        try { serve(c) } catch (_: Throwable) {} finally {
                            runCatching { c.close() }; active.decrementAndGet(); lastUse = System.currentTimeMillis()
                        }
                    }
                }
            }
        }
        val safe = URLEncoder.encode(name.ifBlank { "file" }, "UTF-8").replace("+", "%20")
        return "http://127.0.0.1:${ss.localPort}/$token/$safe"
    }

    @Synchronized
    private fun get(token: String): StreamSource? = items[token]

    @Synchronized
    fun close() {
        items.clear()
        runCatching { server?.close() }
        server = null
    }

    private fun serve(c: Socket) {
        c.soTimeout = 30_000
        val inp = BufferedInputStream(c.getInputStream())
        val out = c.getOutputStream()
        val head = readHead(inp) ?: return
        val req = parseHead(head) ?: return reply(out, 400, "Bad Request")
        if (req.method != "GET" && req.method != "HEAD") return reply(out, 405, "Method Not Allowed")
        val src = get(req.path.removePrefix("/").substringBefore('/')) ?: return reply(out, 404, "Not Found")
        if (req.method == "HEAD") {
            writeHead(out, 200, "OK", listOf("Content-Type" to src.mime, "Content-Length" to "${src.size}", "Accept-Ranges" to "bytes"))
            return
        }
        val rb = Request.Builder().url(src.url)
        src.headers.forEach { (k, v) -> rb.header(k, v) }
        req.range?.let { rb.header("Range", it) }
        val call = rb.build()
        val first = try { client.newCall(call).execute() } catch (e: java.io.IOException) {
            if (!staleConnection(e)) throw e
            client.connectionPool.evictAll()     // the PC dropped a kept-alive connection
            client.newCall(call).execute()
        }
        first.use { resp ->
            if (resp.code != 200 && resp.code != 206 && resp.code != 416) return reply(out, 502, "Bad Gateway")
            val h = ArrayList<Pair<String, String>>()
            h += "Content-Type" to (resp.header("Content-Type") ?: src.mime)
            h += "Accept-Ranges" to "bytes"
            resp.header("Content-Length")?.let { h += "Content-Length" to it }
            resp.header("Content-Range")?.let { h += "Content-Range" to it }
            writeHead(out, resp.code, if (resp.code == 206) "Partial Content" else if (resp.code == 416) "Range Not Satisfiable" else "OK", h)
            resp.body?.byteStream()?.use { i ->
                val buf = ByteArray(256 * 1024)
                while (true) {
                    val n = i.read(buf); if (n < 0) break
                    out.write(buf, 0, n); lastUse = System.currentTimeMillis()
                }
            }
            out.flush()
        }
    }

    private fun reply(out: OutputStream, code: Int, text: String) =
        writeHead(out, code, text, listOf("Content-Length" to "0"))

    private fun writeHead(out: OutputStream, code: Int, text: String, headers: List<Pair<String, String>>) {
        val sb = StringBuilder("HTTP/1.1 $code $text\r\n")
        headers.forEach { (k, v) -> sb.append(k).append(": ").append(v.filter { it >= ' ' }).append("\r\n") }
        sb.append("Connection: close\r\n\r\n")
        out.write(sb.toString().toByteArray(Charsets.ISO_8859_1))
        out.flush()
    }

    companion object {
        const val MAX_ITEMS = 32
        private const val MAX_HEAD = 16 * 1024

        fun defaultClient(): OkHttpClient = OkHttpClient.Builder()
            .connectTimeout(10, TimeUnit.SECONDS).readTimeout(60, TimeUnit.SECONDS).build()

        /** Request line and headers up to the blank line, or null (too long / closed). */
        internal fun readHead(inp: InputStream): String? {
            val buf = ByteArrayOutputStream(512)
            var last4 = 0
            while (true) {
                val b = inp.read()
                if (b < 0) return null
                buf.write(b)
                last4 = (last4 shl 8) or b
                if (last4 == 0x0d0a0d0a) return buf.toString("ISO-8859-1")
                if (buf.size() > MAX_HEAD) return null
            }
        }

        internal data class Head(val method: String, val path: String, val range: String?)

        internal fun parseHead(head: String): Head? {
            val lines = head.split("\r\n")
            val parts = lines.firstOrNull()?.split(' ') ?: return null
            if (parts.size != 3 || !parts[2].startsWith("HTTP/")) return null
            val ranges = lines.drop(1).mapNotNull { l ->
                val i = l.indexOf(':')
                if (i > 0 && l.substring(0, i).trim().equals("Range", ignoreCase = true)) l.substring(i + 1).trim() else null
            }
            // Pass on one single byte range only, nothing else from the player
            // (two Range headers: ignore both and send the whole file).
            val v = ranges.singleOrNull()
            val range = v?.takeIf { Regex("""^bytes=\d*-\d*$""").matches(it) && it != "bytes=-" }
            return Head(parts[0], parts[1], range)
        }

        val shared by lazy { StreamProxy() }
    }
}
