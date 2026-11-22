package com.hafij.harborfiles

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test
import java.io.BufferedReader
import java.io.File
import java.io.IOException
import java.io.InputStream
import java.io.InputStreamReader
import java.net.Socket
import java.nio.file.Files

/** Regression tests for the 2026-11-21 glitch hunt fixes. */
class GlitchFixTest {
    private lateinit var root: File
    private var server: FtpServer? = null

    @Before fun setUp() { root = Files.createTempDirectory("harborfix").toFile() }
    @After fun tearDown() { server?.stop(); root.deleteRecursively() }

    private fun startServer(): FtpServer = FtpServer(root, "123456", 0).also { it.start(); server = it }

    // ---------------------------------------------------------------- names

    @Test fun safeNameKeepsNormalNames() {
        assertEquals("a b.txt", safeName("a b.txt"))
        assertEquals(".hidden", safeName(".hidden"))
        assertEquals("né (1).jpg", safeName("né (1).jpg"))
        assertEquals("_", safeName(".."))
        assertEquals("_", safeName("."))
        assertEquals("_", safeName(""))
        assertEquals(".._.._DCIM_evil.jpg", safeName("../../DCIM/evil.jpg"))
        assertEquals("_etc_x", safeName("/etc/x"))
        assertTrue(isSingleName("a.txt"))
        assertFalse(isSingleName("../a.txt"))
        assertFalse(isSingleName(".."))
        assertFalse(isSingleName("a/b"))
    }

    @Test fun localWriteRefusesEscapingNames() {
        val dir = File(root, "dl").apply { mkdirs() }
        val fs = LocalFs()
        try { fs.write(dir.path, "../evil.txt", 1, "x".byteInputStream()); fail("should refuse") } catch (_: IOException) {}
        assertFalse(File(root, "evil.txt").exists())
        assertFalse(File(root, "..evil.txt.part").exists())
        fs.write(dir.path, "ok.txt", 2, "hi".byteInputStream())
        assertEquals("hi", File(dir, "ok.txt").readText())
    }

    /** A plain folder that says it is local, without Android's storage root. */
    private class DirFs(override val key: String = "dir") : Fs {
        override val title = "dir"
        override val root = "/"
        override val isLocal = true
        override fun list(path: String) = (File(path).listFiles() ?: emptyArray()).map {
            Entry(it.name, it.path, it.isDirectory, it.length())
        }
        override fun mkdir(parent: String, name: String) { if (!File(parent, name).mkdirs()) throw IOException("exists") }
        override fun delete(e: Entry) { File(e.path).deleteRecursively() }
        override fun rename(e: Entry, newName: String) {}
        override fun <T> read(path: String, block: (InputStream) -> T): T = File(path).inputStream().use(block)
        override fun write(dir: String, name: String, size: Long, input: InputStream) =
            LocalFs().write(dir, name, size, input)
    }

    /** A "server" whose listing hands out hostile names. */
    private class EvilFs : Fs {
        override val key = "evil"
        override val title = "evil"
        override val root = "/"
        override fun list(path: String) = listOf(
            Entry("../../escape.txt", "/x", false, 1),
            Entry("..", "/y", false, 1),
        )
        override fun mkdir(parent: String, name: String) {}
        override fun delete(e: Entry) {}
        override fun rename(e: Entry, newName: String) {}
        override fun <T> read(path: String, block: (InputStream) -> T): T = block("data".byteInputStream())
        override fun write(dir: String, name: String, size: Long, input: InputStream) {}
    }

    @Test fun remoteNamesStayInsideDownloadFolder() {
        val dl = File(root, "a/b/dl").apply { mkdirs() }
        val evil = EvilFs()
        val local = DirFs()
        Transfer.copy(evil, Entry("folder", "/", true), local, dl.path) {}
        Transfer.copy(evil, Entry("../../../up", "/", true), local, dl.path) {}
        assertFalse(File(root, "escape.txt").exists())
        assertFalse(File(root, "a/escape.txt").exists())
        assertFalse(File(root, "up").exists())
        assertTrue(File(dl, "folder/.._.._escape.txt").isFile)
        assertTrue(File(dl, "folder/_").isFile)
        assertTrue(File(dl, ".._.._.._up/.._.._escape.txt").isFile)
    }

    // ------------------------------------------------------------ copy/move

    @Test fun copyOrMoveFolderIntoItselfIsRefused() {
        val a = File(root, "A").apply { mkdirs() }
        File(a, "f.txt").writeText("1")
        val sub = File(a, "sub").apply { mkdirs() }
        val fs = DirFs()
        val e = Entry("A", a.path, true)
        for (target in listOf(a, sub)) {
            try { Transfer.copy(fs, e, fs, target.path) {}; fail("copy should refuse") } catch (_: IOException) {}
            try { Transfer.move(fs, e, fs, target.path) {}; fail("move should refuse") } catch (_: IOException) {}
        }
        assertFalse(File(a, "A").exists())
        assertFalse(File(sub, "A").exists())
        // copying beside itself still works and makes "A (1)"
        assertEquals("A (1)", Transfer.copy(fs, e, fs, root.path) {})
        assertEquals("1", File(root, "A (1)/f.txt").readText())
        // a sibling whose name starts the same is not "inside"
        val ab = File(root, "AB").apply { mkdirs() }
        assertEquals("A", Transfer.copy(fs, e, fs, ab.path) {})
        assertTrue(File(ab, "A/f.txt").isFile)
        // remote same server: path prefix check
        val r1 = object : Fs by EvilFs() { override val key = "srv" }
        assertTrue(Transfer.intoItself(r1, Entry("A", "/dav/A/", true), r1, "/dav/A/sub/"))
        assertFalse(Transfer.intoItself(r1, Entry("A", "/dav/A/", true), r1, "/dav/AB/"))
    }

    // ------------------------------------------------------------ FTP server

    @Test fun lineReaderBoundsAndEndings() {
        val r = LineReader("USER a\r\nPASS b\nNOOP\rQUIT\r\n".byteInputStream())
        assertEquals("USER a", r.readLine())
        assertEquals("PASS b", r.readLine())
        assertEquals("NOOP", r.readLine())
        assertEquals("QUIT", r.readLine())
        assertNull(r.readLine())
        assertEquals("né", LineReader("né\r\n".toByteArray().inputStream()).readLine())
        val endless = object : InputStream() { override fun read() = 'A'.code }
        assertNull(LineReader(endless, 8192).readLine())
    }

    @Test fun hugeLineDropsOnlyThatClient() {
        val s = startServer()
        Socket("127.0.0.1", s.boundPort).use { sock ->
            sock.soTimeout = 10_000
            val inp = BufferedReader(InputStreamReader(sock.getInputStream()))
            assertTrue(inp.readLine().startsWith("220"))
            val chunk = ByteArray(64 * 1024) { 'A'.code.toByte() }
            try { repeat(64) { sock.getOutputStream().write(chunk) } } catch (_: IOException) { /* server hung up */ }
            assertNull(try { inp.readLine() } catch (_: IOException) { null })
        }
        val fs = FtpFs("k", "t", "127.0.0.1", s.boundPort, "harbor", "123456")
        File(root, "still.txt").writeText("up")
        assertTrue(fs.list("/").any { it.name == "still.txt" })
    }

    @Test fun pinLockoutIsPerClient() {
        val s = FtpServer(root, "123456", 0)
        repeat(4) { assertEquals("bad", s.checkPin("000000", "10.0.0.9")) }
        assertEquals("locked", s.checkPin("000000", "10.0.0.9"))
        assertEquals("locked", s.checkPin("123456", "10.0.0.9"))
        assertEquals("ok", s.checkPin("123456", "10.0.0.5"))
    }

    /** Minimal raw client: login, PASV, send [data] with [cmd]. */
    private fun upload(port: Int, cmd: String, data: String, rest: Long = -1) {
        Socket("127.0.0.1", port).use { sock ->
            sock.soTimeout = 10_000
            val inp = BufferedReader(InputStreamReader(sock.getInputStream()))
            val out = sock.getOutputStream()
            fun send(c: String): String { out.write("$c\r\n".toByteArray()); out.flush(); return inp.readLine() }
            inp.readLine()
            send("USER harbor"); assertTrue(send("PASS 123456").startsWith("230"))
            send("TYPE I")
            val pasv = send("PASV")
            val n = Regex("""\((\d+),(\d+),(\d+),(\d+),(\d+),(\d+)\)""").find(pasv)!!.groupValues.drop(1).map { it.toInt() }
            if (rest >= 0) assertTrue(send("REST $rest").startsWith("350"))
            out.write("$cmd up.bin\r\n".toByteArray()); out.flush()
            Socket("127.0.0.1", n[4] * 256 + n[5]).use { d ->
                assertTrue(inp.readLine().startsWith("150"))
                d.getOutputStream().write(data.toByteArray())
            }
            assertTrue(inp.readLine().startsWith("226"))
            send("QUIT")
        }
    }

    @Test fun restStorWritesAtOffsetAndAppeStillAppends() {
        val s = startServer()
        val f = File(root, "up.bin")
        f.writeText("abcdefgh")
        upload(s.boundPort, "STOR", "XYZ", rest = 3)
        assertEquals("abcXYZgh", f.readText())
        upload(s.boundPort, "APPE", "!!")
        assertEquals("abcXYZgh!!", f.readText())
        upload(s.boundPort, "STOR", "new")
        assertEquals("new", f.readText())
    }
}
