package com.hafij.harborfiles

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.Test

/** Runs only when HARBOR_DAV="port pin" points at a live Harbor desktop share on 127.0.0.1. */
class DavLiveTest {
    @Test fun againstHarborDesktop() {
        val env = System.getenv("HARBOR_DAV")
        assumeTrue(env != null)
        val (port, pin) = env!!.split(' ')
        val fs = DavFs("d", "PC", "127.0.0.1", port.toInt(), "/dav/", "harbor", pin)
        val top = fs.list(fs.root)
        assertEquals(listOf("Docs"), top.map { it.name })
        val docs = top[0]
        assertTrue(docs.isDir)
        assertEquals("hi\n", fs.read(fs.list(docs.path).first { it.name == "a.txt" }.path) { it.readBytes().decodeToString() })
        val data = "phone → pc ".repeat(5000)
        val bytes = data.toByteArray()
        fs.write(docs.path, "from phone é.txt", bytes.size.toLong(), bytes.inputStream())
        fs.mkdir(docs.path, "New folder")
        val names = fs.list(docs.path).map { it.name }.toSet()
        assertTrue(names.containsAll(setOf("a.txt", "from phone é.txt", "New folder")))
        // DELETE is not checked here: the desktop moves files to the Trash, which needs a real home folder
        val bad = DavFs("d", "PC", "127.0.0.1", port.toInt(), "/dav/", "harbor", "000000")
        try { bad.list(bad.root); throw AssertionError("wrong PIN accepted") } catch (e: java.io.IOException) {
            assertTrue(e.message!!, e.message!!.contains("PIN"))
        }
    }
}
