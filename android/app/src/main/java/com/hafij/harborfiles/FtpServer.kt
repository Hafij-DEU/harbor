package com.hafij.harborfiles

import java.io.BufferedReader
import java.io.File
import java.io.IOException
import java.io.InputStreamReader
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
 *  - 5 wrong PINs lock everyone out for 5 minutes
 *  - every path is jailed inside [root]
 *  - the PIN is never logged
 */
class FtpServer(
    private val root: File,
    private val pin: String,
    private val port: Int,
    private val readOnly: () -> Boolean = { false },
    private val onActivity: () -> Unit = {},
) {
    private var server: ServerSocket? = null
    private val sessions = Collections.synchronizedSet(HashSet<Socket>())
    private val rootPath: String = root.canonicalPath
    @Volatile private var fails = 0
    @Volatile private var lockedUntil = 0L

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
                sessions += s
                thread(name = "ftp-session", isDaemon = true) {
                    try { Session(s).run() } catch (_: Exception) {
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

    private fun checkPin(given: String): String {
        val now = System.currentTimeMillis()
        if (now < lockedUntil) return "locked"
        if (MessageDigest.isEqual(given.toByteArray(), pin.toByteArray())) { fails = 0; return "ok" }
        fails++
        if (fails >= 5) { fails = 0; lockedUntil = now + 5 * 60_000L; return "locked" }
        return "bad"
    }

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
        val inp = BufferedReader(InputStreamReader(sock.getInputStream(), Charsets.UTF_8))
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
            reply(220, "Harbor Files FTP ready")
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
                "PASS" -> when (checkPin(arg.trim())) {
                    "ok" -> { authed = true; reply(230, "Welcome") }
                    "locked" -> reply(421, "Too many wrong PINs. Try again in 5 minutes.")
                    else -> reply(530, "Wrong PIN")
                }
                "AUTH" -> reply(502, "TLS not supported")
                "SYST" -> reply(215, "UNIX Type: L8")
                "FEAT" -> {
                    out.write("211-Features:\r\n UTF8\r\n MLST type*;size*;modify*;\r\n SIZE\r\n MDTM\r\n EPSV\r\n PASV\r\n REST STREAM\r\n")
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
                    val files = if (r.second.isDirectory) r.second.listFiles()?.toList() ?: emptyList() else listOf(r.second)
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
                    val append = cmd == "APPE" || rest > 0; rest = 0
                    receiveData(f, append)
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

        fun receiveData(f: File, append: Boolean) {
            reply(150, "Ready for data")
            val d = openData() ?: run { reply(425, "No data connection"); return }
            try {
                f.parentFile?.mkdirs()
                d.getInputStream().use { i ->
                    java.io.FileOutputStream(f, append).use { o -> i.copyTo(o, 256 * 1024); onActivity() }
                }
            } catch (e: IOException) {
                reply(426, "Transfer aborted"); return
            } finally { runCatching { d.close() } }
            reply(226, "Saved")
        }
    }

    companion object {
        private fun stamp(ms: Long): String = SimpleDateFormat("yyyyMMddHHmmss", Locale.US)
            .apply { timeZone = TimeZone.getTimeZone("UTC") }.format(Date(ms))

        fun mlsx(f: File): String =
            if (f.isDirectory) "type=dir;modify=${stamp(f.lastModified())};"
            else "type=file;size=${f.length()};modify=${stamp(f.lastModified())};"

        fun lsLine(f: File): String {
            val ms = f.lastModified()
            val recent = System.currentTimeMillis() - ms < 180L * 86_400_000L
            val date = SimpleDateFormat(if (recent) "MMM dd HH:mm" else "MMM dd  yyyy", Locale.US).format(Date(ms))
            val perm = if (f.isDirectory) "drwxr-xr-x" else "-rw-r--r--"
            val size = if (f.isDirectory) 0 else f.length()
            return String.format(Locale.US, "%s 1 owner group %13d %s %s\r\n", perm, size, date, f.name)
        }
    }
}
