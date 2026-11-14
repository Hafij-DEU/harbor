package com.hafij.harborfiles

import okhttp3.Credentials
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody
import okhttp3.RequestBody.Companion.toRequestBody
import okio.BufferedSink
import okio.source
import org.w3c.dom.Element
import java.io.BufferedReader
import java.io.IOException
import java.io.InputStream
import java.io.InputStreamReader
import java.io.OutputStreamWriter
import java.io.Writer
import java.net.InetSocketAddress
import java.net.Socket
import java.net.URLDecoder
import java.net.URLEncoder
import java.text.SimpleDateFormat
import java.util.Locale
import java.util.TimeZone
import java.util.concurrent.TimeUnit
import javax.xml.parsers.DocumentBuilderFactory

/** A saved PC. The password lives in [Secret], never here. */
data class Server(
    val id: String,
    val type: String, // "dav" or "ftp"
    val name: String,
    val host: String,
    val port: Int,
    val user: String,
    val path: String = "",
) {
    fun encode() = listOf(id, type, name, host, port.toString(), user, path).joinToString("\t") { it.replace("\t", " ") }

    companion object {
        fun decode(l: String): Server? {
            val p = l.split('\t')
            if (p.size < 6) return null
            return Server(p[0], p[1], p[2], p[3], p[4].toIntOrNull() ?: return null, p[5], p.getOrElse(6) { "" })
        }
    }
}

fun fsFor(s: Server, password: String): Fs =
    if (s.type == "ftp") FtpFs(s.id, s.name, s.host, s.port, s.user.ifEmpty { "harbor" }, password)
    else DavFs(s.id, s.name, s.host, s.port, s.path.ifEmpty { "/dav/" }, s.user.ifEmpty { "harbor" }, password)

// ---------------------------------------------------------------- FTP client

class FtpFs(
    override val key: String,
    override val title: String,
    private val host: String,
    private val port: Int,
    private val user: String,
    private val pass: String,
) : Fs {
    override val root = "/"

    private inner class Conn : AutoCloseable {
        val sock = Socket().apply { connect(InetSocketAddress(this@FtpFs.host, this@FtpFs.port), 10_000); soTimeout = 60_000 }
        val inp = BufferedReader(InputStreamReader(sock.getInputStream(), Charsets.UTF_8))
        val out: Writer = OutputStreamWriter(sock.getOutputStream(), Charsets.UTF_8)

        init {
            expect(read(), 2)
            var r = cmd("USER $user")
            if (r.first == 331) r = cmd("PASS $pass")
            if (r.first !in 200..299) throw IOException(if (r.first == 421) r.second else "Login failed: ${r.second}")
            cmd("OPTS UTF8 ON")
            expect(cmd("TYPE I"), 2)
        }

        fun read(): Pair<Int, String> {
            var line = inp.readLine() ?: throw IOException("Server closed the connection")
            val code = line.take(3).toIntOrNull() ?: throw IOException("Bad reply: $line")
            if (line.length > 3 && line[3] == '-') {
                val end = "$code "
                while (true) { line = inp.readLine() ?: break; if (line.startsWith(end)) break }
            }
            return code to line.drop(4)
        }

        fun cmd(c: String): Pair<Int, String> { out.write(c + "\r\n"); out.flush(); return read() }

        fun expect(r: Pair<Int, String>, cls: Int): Pair<Int, String> {
            if (r.first / 100 != cls) throw IOException(r.second.ifEmpty { "FTP error ${r.first}" })
            return r
        }

        /** PASV, but always connect to the host we know (servers behind NAT lie about their IP). */
        fun data(): Socket {
            val r = expect(cmd("PASV"), 2)
            val nums = Regex("""(\d+),(\d+),(\d+),(\d+),(\d+),(\d+)""").find(r.second)?.groupValues
                ?: throw IOException("Bad PASV reply")
            val p = nums[5].toInt() * 256 + nums[6].toInt()
            return Socket().apply { connect(InetSocketAddress(this@FtpFs.host, p), 10_000); soTimeout = 60_000 }
        }

        override fun close() { runCatching { cmd("QUIT") }; runCatching { sock.close() } }
    }

    private fun <T> conn(block: Conn.() -> T): T = Conn().use { it.block() }

    override fun list(path: String): List<Entry> = conn {
        val d = data()
        var r = cmd("MLSD $path")
        val mlsd = r.first / 100 == 1
        if (!mlsd) {
            runCatching { d.close() }
            val d2 = data()
            r = expect(cmd("LIST -a $path"), 1)
            val text = d2.getInputStream().bufferedReader(Charsets.UTF_8).use { it.readText() }
            d2.close(); read()
            return@conn parseLs(text, path).filter { it.name != "." && it.name != ".." }
        }
        val text = d.getInputStream().bufferedReader(Charsets.UTF_8).use { it.readText() }
        d.close(); read()
        parseMlsd(text, path)
    }

    override fun mkdir(parent: String, name: String) { conn { expect(cmd("MKD ${join(parent, name)}"), 2) } }

    override fun delete(e: Entry) {
        if (e.isDir) {
            list(e.path).forEach { delete(it) }
            conn { expect(cmd("RMD ${e.path}"), 2) }
        } else conn { expect(cmd("DELE ${e.path}"), 2) }
    }

    override fun rename(e: Entry, newName: String) {
        conn {
            expect(cmd("RNFR ${e.path}"), 3)
            expect(cmd("RNTO ${join(e.path.substringBeforeLast('/').ifEmpty { "/" }, newName)}"), 2)
        }
    }

    override fun <T> read(path: String, block: (InputStream) -> T): T = conn {
        val d = data()
        expect(cmd("RETR $path"), 1)
        val res = d.getInputStream().use(block)
        d.close(); read()
        res
    }

    override fun write(dir: String, name: String, size: Long, input: InputStream) {
        conn {
            val d = data()
            expect(cmd("STOR ${join(dir, name)}"), 1)
            d.getOutputStream().use { input.copyTo(it, 256 * 1024) }
            d.close(); expect(read(), 2)
        }
    }

    companion object {
        private fun mlsdTime(s: String): Long = runCatching {
            SimpleDateFormat("yyyyMMddHHmmss", Locale.US).apply { timeZone = TimeZone.getTimeZone("UTC") }
                .parse(s.take(14))!!.time
        }.getOrDefault(0)

        fun parseMlsd(text: String, dir: String): List<Entry> = text.lineSequence().mapNotNull { l ->
            val sp = l.indexOf(' ')
            if (sp < 0) return@mapNotNull null
            val facts = l.substring(0, sp).split(';').mapNotNull {
                val i = it.indexOf('='); if (i < 0) null else it.substring(0, i).lowercase() to it.substring(i + 1)
            }.toMap()
            val name = l.substring(sp + 1).trimEnd('\r')
            val type = facts["type"]?.lowercase() ?: return@mapNotNull null
            if (type == "cdir" || type == "pdir") return@mapNotNull null
            val p = if (dir.endsWith("/")) dir + name else "$dir/$name"
            Entry(name, p, type == "dir", facts["size"]?.toLongOrNull() ?: 0, mlsdTime(facts["modify"] ?: ""))
        }.toList()

        private val LS = Regex("""^([\-dlbcps])\S*\s+\d+\s+\S+\s+\S+\s+(\d+)\s+(\w{3}\s+\d+\s+[\d:]+)\s(.+)$""")

        fun parseLs(text: String, dir: String): List<Entry> = text.lineSequence().mapNotNull { l ->
            val m = LS.find(l.trimEnd('\r')) ?: return@mapNotNull null
            var name = m.groupValues[4]
            if (m.groupValues[1] == "l") name = name.substringBefore(" -> ")
            val p = if (dir.endsWith("/")) dir + name else "$dir/$name"
            Entry(name, p, m.groupValues[1] == "d" || m.groupValues[1] == "l", m.groupValues[2].toLongOrNull() ?: 0, 0)
        }.toList()
    }
}

// ------------------------------------------------------------- WebDAV client

class DavFs(
    override val key: String,
    override val title: String,
    host: String,
    port: Int,
    davPath: String,
    user: String,
    pass: String,
) : Fs {
    private val origin = "http://$host:$port"
    override val root = "/" + davPath.trim('/').let { if (it.isEmpty()) "" else "$it/" }
    private val auth = Credentials.basic(user, pass, Charsets.UTF_8)
    private val http = OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS).readTimeout(60, TimeUnit.SECONDS).writeTimeout(60, TimeUnit.SECONDS)
        .build()

    override fun join(dir: String, name: String) =
        (if (dir.endsWith("/")) dir else "$dir/") + URLEncoder.encode(name, "UTF-8").replace("+", "%20")

    override fun nameOf(path: String) = URLDecoder.decode(path.trimEnd('/').substringAfterLast('/').replace("+", "%2B"), "UTF-8")

    private fun req(path: String) = Request.Builder().url(origin + path).header("Authorization", auth)

    private fun call(r: Request): okhttp3.Response {
        val resp = http.newCall(r).execute()
        if (!resp.isSuccessful) {
            val msg = when (resp.code) {
                401 -> "Wrong PIN"
                429 -> "Too many wrong PINs. Try again in 5 minutes."
                403 -> "Not allowed (read-only folder?)"
                405 -> "The server doesn't allow that"
                else -> "Server error ${resp.code}"
            }
            resp.close(); throw IOException(msg)
        }
        return resp
    }

    override fun list(path: String): List<Entry> {
        val body = """<?xml version="1.0"?><D:propfind xmlns:D="DAV:"><D:allprop/></D:propfind>"""
            .toRequestBody("application/xml".toMediaType())
        val xml = call(req(path).method("PROPFIND", body).header("Depth", "1").build()).use { it.body!!.string() }
        return parseMultistatus(xml, path)
    }

    override fun mkdir(parent: String, name: String) {
        call(req(join(parent, name) + "/").method("MKCOL", null).build()).close()
    }

    override fun delete(e: Entry) { call(req(e.path).delete().build()).close() }

    override fun rename(e: Entry, newName: String) {
        val parent = e.path.trimEnd('/').substringBeforeLast('/') + "/"
        val dest = origin + join(parent, newName) + if (e.isDir) "/" else ""
        call(req(e.path).method("MOVE", null).header("Destination", dest).header("Overwrite", "F").build()).close()
    }

    override fun <T> read(path: String, block: (InputStream) -> T): T =
        call(req(path).get().build()).use { block(it.body!!.byteStream()) }

    override fun write(dir: String, name: String, size: Long, input: InputStream) {
        val body = object : RequestBody() {
            override fun contentType() = "application/octet-stream".toMediaType()
            override fun contentLength() = if (size > 0) size else -1
            override fun writeTo(sink: BufferedSink) { sink.writeAll(input.source()) }
        }
        call(req(join(dir, name)).put(body).build()).close()
    }

    companion object {
        private fun httpDate(s: String): Long = runCatching {
            SimpleDateFormat("EEE, dd MMM yyyy HH:mm:ss zzz", Locale.US).parse(s)!!.time
        }.getOrDefault(0)

        private fun Element.first(tag: String): String? {
            val n = getElementsByTagNameNS("DAV:", tag)
            return if (n.length > 0) n.item(0).textContent else null
        }

        fun parseMultistatus(xml: String, self: String): List<Entry> {
            val f = DocumentBuilderFactory.newInstance().apply { isNamespaceAware = true }
            runCatching { f.setFeature("http://apache.org/xml/features/disallow-doctype-decl", true) }
            val doc = f.newDocumentBuilder().parse(xml.byteInputStream())
            val rs = doc.getElementsByTagNameNS("DAV:", "response")
            val selfNorm = URLDecoder.decode(self, "UTF-8").trimEnd('/')
            val out = ArrayList<Entry>()
            for (i in 0 until rs.length) {
                val r = rs.item(i) as Element
                var href = r.first("href") ?: continue
                if (href.startsWith("http://") || href.startsWith("https://")) href = "/" + href.substringAfter("://").substringAfter('/', "")
                if (URLDecoder.decode(href, "UTF-8").trimEnd('/') == selfNorm) continue
                val isDir = r.getElementsByTagNameNS("DAV:", "collection").length > 0
                val name = r.first("displayname")?.takeIf { it.isNotEmpty() }
                    ?: URLDecoder.decode(href.trimEnd('/').substringAfterLast('/'), "UTF-8")
                out += Entry(
                    name, href, isDir,
                    r.first("getcontentlength")?.trim()?.toLongOrNull() ?: 0,
                    r.first("getlastmodified")?.let { httpDate(it.trim()) } ?: 0,
                    mime = r.first("getcontenttype"),
                )
            }
            return out
        }
    }
}
