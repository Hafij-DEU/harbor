package com.hafij.harborfiles

import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Before
import org.junit.Test
import java.io.File
import java.nio.file.Files

class HarborTest {
    private lateinit var root: File
    private var server: FtpServer? = null

    @Before fun setUp() { root = Files.createTempDirectory("harbor").toFile() }
    @After fun tearDown() { server?.stop(); root.deleteRecursively() }

    private fun start(readOnly: Boolean = false): FtpFs {
        val s = FtpServer(root, "123456", 0, { readOnly })
        s.start(); server = s
        return FtpFs("k", "t", "127.0.0.1", s.boundPort, "harbor", "123456")
    }

    @Test fun ftpRoundTrip() {
        File(root, "a.txt").writeText("hello")
        File(root, "sub").mkdir()
        val fs = start()
        val names = fs.list("/").map { it.name }.toSet()
        assertEquals(setOf("a.txt", "sub"), names)
        assertTrue(fs.list("/").first { it.name == "sub" }.isDir)
        assertEquals("hello", fs.read("/a.txt") { it.readBytes().decodeToString() })

        val data = "x".repeat(200_000)
        fs.write("/sub", "big file.bin", data.length.toLong(), data.byteInputStream())
        assertEquals(data, File(root, "sub/big file.bin").readText())

        fs.mkdir("/", "new")
        assertTrue(File(root, "new").isDirectory)
        fs.rename(fs.list("/").first { it.name == "a.txt" }, "b.txt")
        assertTrue(File(root, "b.txt").exists())
        fs.delete(fs.list("/").first { it.name == "b.txt" })
        assertFalse(File(root, "b.txt").exists())
        fs.delete(fs.list("/").first { it.name == "new" })
        assertFalse(File(root, "new").exists())
    }

    @Test fun wrongPinRejected() {
        start()
        val bad = FtpFs("k", "t", "127.0.0.1", server!!.boundPort, "harbor", "000000")
        try { bad.list("/"); fail("should fail") } catch (e: Exception) { }
    }

    @Test fun readOnlyBlocksWrites() {
        val fs = start(readOnly = true)
        try { fs.mkdir("/", "x"); fail("should fail") } catch (e: Exception) { }
        assertFalse(File(root, "x").exists())
    }

    @Test fun pathJail() {
        val s = FtpServer(root, "123456", 0)
        // ".." stops at the share root, like a chroot
        assertEquals(File(root, "etc/passwd").canonicalPath, s.resolve("/", "../../etc/passwd")?.second?.canonicalPath)
        assertEquals("/", s.resolve("/", "/../..")?.first)
        // a symlink that points outside the root is refused
        Files.createSymbolicLink(File(root, "out").toPath(), File("/etc").toPath())
        assertNull(s.resolve("/", "out/passwd"))
        val ok = s.resolve("/", "sub/../a.txt")
        assertEquals(File(root, "a.txt").canonicalPath, ok?.second?.canonicalPath)
    }

    @Test fun trashPutRestoreExpire() {
        val t = Trash(File(root, ".HarborTrash"))
        val f = File(root, "doc.txt").apply { writeText("1") }
        val item = t.put(f, now = 1_000)
        assertFalse(f.exists())
        assertEquals(1, t.list().size)
        File(root, "doc.txt").writeText("other")
        val back = t.restore(item)
        assertEquals("doc (1).txt", back.name)
        assertEquals("1", back.readText())
        assertTrue(t.list().isEmpty())

        t.put(File(root, "doc.txt"), now = 0)
        assertEquals(1, t.expire(30, now = 31L * 86_400_000L))
        assertTrue(t.list().isEmpty())
    }

    @Test fun uniqueNames() {
        val taken = setOf("a.txt", "a (1).txt")
        assertEquals("a (2).txt", uniqueName("a.txt") { it in taken })
        assertEquals("b.txt", uniqueName("b.txt") { it in taken })
        assertEquals(".hidden (1)", uniqueName(".hidden") { it == ".hidden" })
    }

    @Test fun parseListings() {
        val mlsd = "type=dir;modify=20260101120000; Photos\r\ntype=file;size=42;modify=20260102120000; a b.txt\r\ntype=cdir; .\r\n"
        val m = FtpFs.parseMlsd(mlsd, "/x")
        assertEquals(listOf("Photos", "a b.txt"), m.map { it.name })
        assertEquals(42L, m[1].size)
        assertEquals("/x/a b.txt", m[1].path)
        val ls = "drwxr-xr-x 1 owner group 0 Jan 01 12:00 Music\r\n-rw-r--r-- 1 owner group 1234 Mar 05  2025 song 1.mp3\r\n"
        val l = FtpFs.parseLs(ls, "/")
        assertEquals(listOf("Music", "song 1.mp3"), l.map { it.name })
        assertTrue(l[0].isDir)
        assertEquals(1234L, l[1].size)
    }

    @Test fun parseDav() {
        val xml = """<?xml version="1.0" encoding="utf-8"?>
<d:multistatus xmlns:d="DAV:">
 <d:response><d:href>/dav/Home/</d:href><d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop></d:propstat></d:response>
 <d:response><d:href>/dav/Home/My%20Docs/</d:href><d:propstat><d:prop><d:displayname>My Docs</d:displayname><d:resourcetype><d:collection/></d:resourcetype><d:getlastmodified>Mon, 02 Nov 2026 10:00:00 GMT</d:getlastmodified></d:prop></d:propstat></d:response>
 <d:response><d:href>/dav/Home/n%C3%A9.txt</d:href><d:propstat><d:prop><d:resourcetype/><d:getcontentlength>7</d:getcontentlength></d:prop></d:propstat></d:response>
</d:multistatus>"""
        val l = DavFs.parseMultistatus(xml, "/dav/Home/")
        assertEquals(listOf("My Docs", "né.txt"), l.map { it.name })
        assertTrue(l[0].isDir)
        assertEquals(7L, l[1].size)
    }
}
