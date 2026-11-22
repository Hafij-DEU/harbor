package com.hafij.harborfiles

import java.io.File
import java.io.IOException
import java.io.OutputStream
import java.io.OutputStreamWriter
import java.io.Writer
import java.net.Inet4Address
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.net.Socket
import java.net.SocketTimeoutException
import java.security.MessageDigest
import java.text.SimpleDateFormat
import java.util.Collections
import java.util.Date
import java.util.Locale
import java.util.TimeZone
import kotlin.concurrent.thread

/**
 * Small FTP server for sharing the phone over Wi-Fi.
 *  - any user name, password = the PIN shown on the phone
 *  - 5 wrong PINs lock that device (IP address) out for 5 minutes
 *  - every path is jailed inside [root]
 *  - the PIN is never logged
 */
class FtpServer(
    private val root: File,
    private val pin: String,
    private val port: Int,
    private val readOnly: () -> Boolean = { false },
    private val onActivity: () -> Unit = {},
    deviceName: String = "",
) {
    /** Sent in the 220 greeting so Harbor on the PC can show "Redmi Pad 2". */
    private val greetingName: String = cleanName(deviceName)
    private var server: ServerSocket? = null
    private val sessions = Collections.synchronizedSet(HashSet<Socket>())
    private val rootPath: String = root.canonicalPath
    /** Wrong-PIN count and lock time per client IP, so one bad device can't lock out the rest. */
    private class Lock(var fails: Int = 0, var lockedUntil: Long = 0L)
    private val locks = HashMap<String, Lock>()

    val boundPort: Int get() = server?.localPort ?: -1
    val activeSessions: Int get() = sessions.size

    fun start() {
        val ss = ServerSocket()
        ss.reuseAddress = true
        ss.bind(InetSocketAddress(port))
        server = ss
        thread(name = "ftp-accept", isDaemon = true) {
            while (!ss.isClosed) {
                val s = try { ss.accept() } catch (_: IOException) { break }
                if (sessions.size >= MAX_SESSIONS) {
                    runCatching {
                        s.getOutputStream().write("421 Too many connections, try again later\r\n".toByteArray())
                        s.close()
                    }
                    continue
                }
                sessions += s
                thread(name = "ftp-session", isDaemon = true) {
                    // Throwable, not just Exception: nothing a client sends may
                    // take the whole app down (e.g. an OutOfMemoryError).
                    try { Session(s).run() } catch (_: Throwable) {
                    } finally { sessions -= s; runCatching { s.close() } }
                }
            }
        }
    }

    fun stop() {
        runCatching { server?.close() }
        server = null
        synchronized(sessions) { sessions.forEach { runCatching { it.close() } }; sessions.clear() }
    }

    internal fun checkPin(given: String, client: String = ""): String { synchronized(locks) {
        val now = System.currentTimeMillis()
        locks.entries.removeAll { it.value.fails == 0 && it.value.lockedUntil <= now }
        val l = locks.getOrPut(client) { Lock() }
        if (now < l.lockedUntil) return "locked"
        if (MessageDigest.isEqual(given.toByteArray(), pin.toByteArray())) { l.fails = 0; return "ok" }
        l.fails++
        if (l.fails >= 5) { l.fails = 0; l.lockedUntil = now + 5 * 60_000L; return "locked" }
        return "bad"
    } }

    /** Virtual "/a/b" (relative to cwd) to a real file inside the root, or null. */
    internal fun resolve(cwd: String, arg: String): Pair<String, File>? {
        val raw = if (arg.startsWith("/")) arg else "$cwd/$arg"
        val parts = ArrayList<String>()
        for (p in raw.split('/')) when (p) {
            "", "." -> {}
            ".." -> if (parts.isNotEmpty()) parts.removeAt(parts.size - 1)
            else -> parts += p
        }
        val virt = "/" + parts.joinToString("/")
        val f = File(root, parts.joinToString(File.separator))
        val canon = try { f.canonicalPath } catch (_: IOException) { return null }
        if (canon != rootPath && !canon.startsWith(rootPath + File.separator)) return null
        return virt to f
    }

    private inner class Session(val sock: Socket) {
        val inp = LineReader(java.io.BufferedInputStream(sock.getInputStream()))
        val out: Writer = OutputStreamWriter(sock.getOutputStream(), Charsets.UTF_8)
        var authed = false
        var cwd = "/"
        var pasv: ServerSocket? = null
        var active: InetSocketAddress? = null
        var renameFrom: File? = null
        var rest = 0L

        fun reply(code: Int, msg: String) { out.write("$code $msg\r\n"); out.flush() }

        fun run() {
            sock.soTimeout = 10 * 60_000
            reply(220, if (greetingName.isEmpty()) "Harbor Files FTP ready"
                       else "Harbor Files FTP ready ($greetingName)")
            while (true) {
                val line = try { inp.readLine() } catch (_: SocketTimeoutException) { null } ?: break
                onActivity()
                val sp = line.indexOf(' ')
                val cmd = (if (sp < 0) line else line.substring(0, sp)).uppercase(Locale.ROOT)
                val arg = if (sp < 0) "" else line.substring(sp + 1)
                if (!authed && cmd !in setOf("USER", "PASS", "QUIT", "FEAT", "SYST", "OPTS", "NOOP", "AUTH")) {
                    reply(530, "Log in with the PIN shown on the phone"); continue
                }
                try {
                    if (!handle(cmd, arg)) break
                } catch (e: IOException) {
                    reply(451, "Failed: ${e.message ?: "I/O error"}")
                }
            }
            closeData()
        }

        fun handle(cmd: String, arg: String): Boolean {
            when (cmd) {
                "USER" -> reply(331, "PIN please")
                "PASS" -> when (checkPin(arg.trim(), sock.inetAddress?.hostAddress ?: "")) {
                    "ok" -> { authed = true; reply(230, "Welcome") }
                    "locked" -> reply(421, "Too many wrong PINs. Try again in 5 minutes.")
                    else -> reply(530, "Wrong PIN")
                }
                "AUTH" -> reply(502, "TLS not supported")
                "SYST" -> reply(215, "UNIX Type: L8")
                "FEAT" -> {
                    out.write("211-Features:\r\n UTF8\r\n MLST type*;size*;modify*;\r\n SIZE\r\n EPSV\r\n PASV\r\n REST STREAM\r\n")
                    reply(211, "End")
                }
                "OPTS" -> reply(200, "OK")
                "NOOP" -> reply(200, "OK")
                "TYPE", "MODE", "STRU" -> reply(200, "OK")
                "PWD", "XPWD" -> reply(257, "\"${cwd.replace("\"", "\"\"")}\" is the current folder")
                "CWD", "XCWD" -> {
                    val r = resolve(cwd, arg)
                    if (r != null && r.second.isDirectory) { cwd = r.first; reply(250, "OK") } else reply(550, "No such folder")
                }
                "CDUP", "XCUP" -> { cwd = resolve(cwd, "..")?.first ?: "/"; reply(250, "OK") }
                "PASV" -> {
                    closeData()
                    val local = sock.localAddress
                    val ip = if (local is Inet4Address) local else InetAddress.getByName("127.0.0.1")
                    val ss = ServerSocket(0, 1, ip).also { it.soTimeout = 20_000 }
                    pasv = ss
                    val p = ss.localPort
                    reply(227, "Entering Passive Mode (${ip.hostAddress!!.replace('.', ',')},${p shr 8},${p and 255})")
                }
                "EPSV" -> {
                    closeData()
                    val ss = ServerSocket(0, 1, sock.localAddress).also { it.soTimeout = 20_000 }
                    pasv = ss
                    reply(229, "Entering Extended Passive Mode (|||${ss.localPort}|)")
                }
                "PORT" -> {
                    val n = arg.split(',').mapNotNull { it.trim().toIntOrNull() }
                    if (n.size != 6) { reply(501, "Bad PORT"); return true }
                    val host = n.take(4).joinToString(".")
                    if (InetAddress.getByName(host) != sock.inetAddress) { reply(500, "PORT must point to you"); return true }
                    closeData(); active = InetSocketAddress(host, n[4] * 256 + n[5]); reply(200, "OK")
                }
                "LIST", "NLST", "MLSD" -> {
                    val a = arg.split(' ').filterNot { it.startsWith("-") }.joinToString(" ")
                    val r = resolve(cwd, a)
                    if (r == null || !r.second.exists()) { reply(550, "No such folder"); return true }
                    // One stat per entry: File.isDirectory/length/lastModified each
                    // go through Android's slow storage FUSE layer, so a big
                    // camera folder took 10+ s to list.
                    val files = entries(r.second)
                    val text = buildString {
                        for (f in files.sortedBy { it.name.lowercase() }) append(
                            when (cmd) {
                                "NLST" -> f.name + "\r\n"
                                "MLSD" -> mlsx(f) + " " + f.name + "\r\n"
                                else -> lsLine(f)
                            }
                        )
                    }
                    sendData { it.write(text.toByteArray(Charsets.UTF_8)) }
                }
                "MLST" -> {
                    val r = resolve(cwd, arg)
                    if (r == null || !r.second.exists()) { reply(550, "Not found"); return true }
                    out.write("250-Listing\r\n ${mlsx(r.second)} ${r.first}\r\n"); reply(250, "End")
                }
                "SIZE" -> {
                    val f = resolve(cwd, arg)?.second
                    if (f != null && f.isFile) reply(213, f.length().toString()) else reply(550, "Not a file")
                }
                "MDTM" -> {
                    val f = resolve(cwd, arg)?.second
                    if (f != null && f.exists()) reply(213, stamp(f.lastModified())) else reply(550, "Not found")
                }
                "REST" -> { rest = arg.trim().toLongOrNull() ?: 0; reply(350, "Restarting at $rest") }
                "RETR" -> {
                    val f = resolve(cwd, arg)?.second
                    if (f == null || !f.isFile) { reply(550, "Not a file"); return true }
                    val skip = rest; rest = 0
                    sendData { o -> f.inputStream().use { i -> if (skip > 0) i.skip(skip); i.copyTo(o, 256 * 1024) } }
                }
                "STOR", "APPE" -> {
                    if (readOnly()) { reply(550, "The phone is shared read-only"); return true }
                    val f = resolve(cwd, arg)?.second
                    if (f == null || f.isDirectory) { reply(553, "Bad file name"); return true }
                    // APPE appends; REST n + STOR writes from byte n (resume).
                    val append = cmd == "APPE"
                    val offset = if (append) 0L else rest
                    rest = 0
                    receiveData(f, append, offset)
                }
                "DELE", "RMD", "XRMD", "MKD", "XMKD", "RNFR", "RNTO" -> {
                    if (readOnly()) { reply(550, "The phone is shared read-only"); return true }
                    val r = resolve(cwd, arg)
                    if (r == null || r.second.canonicalPath == rootPath) { reply(550, "Not allowed"); return true }
                    val f = r.second
                    when (cmd) {
                        "DELE" -> if (f.isFile && f.delete()) reply(250, "Deleted") else reply(550, "Can't delete")
                        "RMD", "XRMD" -> if (f.isDirectory && f.deleteRecursively()) reply(250, "Deleted") else reply(550, "Can't delete")
                        "MKD", "XMKD" -> if (f.mkdirs()) reply(257, "\"${r.first}\" created") else reply(550, "Can't create")
                        "RNFR" -> if (f.exists()) { renameFrom = f; reply(350, "Ready") } else reply(550, "Not found")
                        "RNTO" -> {
                            val from = renameFrom; renameFrom = null
                            if (from != null && !f.exists() && from.renameTo(f)) reply(250, "Renamed") else reply(553, "Can't rename")
                        }
                    }
                }
                "ABOR" -> { closeData(); reply(226, "Aborted") }
                "QUIT" -> { reply(221, "Bye"); return false }
                else -> reply(502, "Not supported")
            }
            return true
        }

        fun openData(): Socket? {
            val ss = pasv
            val act = active
            return try {
                when {
                    ss != null -> ss.accept()
                    act != null -> Socket().apply { connect(act, 15_000) }
                    else -> null
                }
            } catch (_: IOException) { null } finally { closeData() }
        }

        fun closeData() { runCatching { pasv?.close() }; pasv = null; active = null }

        fun sendData(body: (OutputStream) -> Unit) {
            reply(150, "Opening data connection")
            val d = openData() ?: run { reply(425, "No data connection"); return }
            try { d.getOutputStream().use { o -> body(o); o.flush() } } catch (e: IOException) {
                reply(426, "Transfer aborted"); return
            } finally { runCatching { d.close() } }
            reply(226, "Done")
        }

        fun receiveData(f: File, append: Boolean, offset: Long = 0L) {
            reply(150, "Ready for data")
            val d = openData() ?: run { reply(425, "No data connection"); return }
            try {
                f.parentFile?.mkdirs()
                d.getInputStream().use { i ->
                    if (offset > 0) {
                        java.io.RandomAccessFile(f, "rw").use { raf ->
                            raf.seek(offset)
                            val buf = ByteArray(256 * 1024)
                            while (true) { val n = i.read(buf); if (n < 0) break; raf.write(buf, 0, n) }
                            onActivity()
                        }
                    } else {
                        java.io.FileOutputStream(f, append).use { o -> i.copyTo(o, 256 * 1024); onActivity() }
                    }
                }
            } catch (e: IOException) {
                reply(426, "Transfer aborted"); return
            } finally { runCatching { d.close() } }
            reply(226, "Saved")
        }
    }

    companion object {
        /** Open control connections at once; more get "421" instead of a thread each. */
        const val MAX_SESSIONS = 32
        /** Longest command line accepted (FTP lines are short); longer drops the client. */
        const val MAX_LINE = 8192

        // Thread-safe formatters made once (sessions run on many threads),
        // not a new SimpleDateFormat for every listed file.
        private val stampFmt = java.time.format.DateTimeFormatter.ofPattern("yyyyMMddHHmmss", Locale.US)
            .withZone(java.time.ZoneOffset.UTC)
        private val recentFmt = java.time.format.DateTimeFormatter.ofPattern("MMM dd HH:mm", Locale.US)
            .withZone(java.time.ZoneId.systemDefault())
        private val oldFmt = java.time.format.DateTimeFormatter.ofPattern("MMM dd  yyyy", Locale.US)
            .withZone(java.time.ZoneId.systemDefault())

        private fun stamp(ms: Long): String = stampFmt.format(java.time.Instant.ofEpochMilli(ms))

        class Entry(val name: String, val isDir: Boolean, val size: Long, val ms: Long)

        fun entry(f: File, name: String = f.name): Entry = try {
            val a = java.nio.file.Files.readAttributes(f.toPath(),
                java.nio.file.attribute.BasicFileAttributes::class.java)
            Entry(name, a.isDirectory, if (a.isDirectory) 0 else a.size(), a.lastModifiedTime().toMillis())
        } catch (e: Exception) {
            Entry(name, f.isDirectory, f.length(), f.lastModified())
        }

        fun entries(dir: File): List<Entry> {
            if (!dir.isDirectory) return listOf(entry(dir))
            val names = dir.listFiles() ?: return emptyList()
            if (names.size < 64) return names.map { entry(it) }
            // Stat big folders on a few threads: each lookup through the
            // storage layer waits on its own round trip, not on the CPU.
            return names.asList().parallelStream().map { entry(it) }
                .collect(java.util.stream.Collectors.toList())
        }

        fun mlsx(f: File): String = mlsx(entry(f))

        fun mlsx(e: Entry): String =
            if (e.isDir) "type=dir;modify=${stamp(e.ms)};"
            else "type=file;size=${e.size};modify=${stamp(e.ms)};"

        fun lsLine(e: Entry): String {
            val recent = System.currentTimeMillis() - e.ms < 180L * 86_400_000L
            val date = (if (recent) recentFmt else oldFmt).format(java.time.Instant.ofEpochMilli(e.ms))
            val perm = if (e.isDir) "drwxr-xr-x" else "-rw-r--r--"
            return String.format(Locale.US, "%s 1 owner group %13d %s %s\r\n", perm, e.size, date, e.name)
        }
    }
}

/** Name safe for one FTP reply line: no brackets or control chars, max 60. */
internal fun cleanName(name: String): String =
    name.filter { it >= ' ' && it != '(' && it != ')' }.trim().take(60).trim()

/**
 * Reads FTP control lines as UTF-8 without the CR/LF, like
 * BufferedReader.readLine (ends on LF, CR or CRLF), but gives up with null
 * once a line passes [max] bytes, so a client that never sends a newline
 * can't fill the memory. Null also at end of stream.
 */
internal class LineReader(private val inp: java.io.InputStream, private val max: Int = FtpServer.MAX_LINE) {
    private var skipLf = false

    fun readLine(): String? {
        val buf = java.io.ByteArrayOutputStream(128)
        while (true) {
            val b = inp.read()
            if (b < 0) return if (buf.size() == 0) null else buf.toString("UTF-8")
            if (skipLf) { skipLf = false; if (b == '\n'.code) continue }
            if (b == '\n'.code) return buf.toString("UTF-8")
            if (b == '\r'.code) { skipLf = true; return buf.toString("UTF-8") }
            if (buf.size() >= max) return null
            buf.write(b)
        }
    }
}
