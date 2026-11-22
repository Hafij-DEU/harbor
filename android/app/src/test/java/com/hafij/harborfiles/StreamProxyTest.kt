package com.hafij.harborfiles

import java.net.HttpURLConnection
import java.net.InetAddress
import java.net.ServerSocket
import kotlin.concurrent.thread
import java.net.URL
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

class StreamProxyTest {
    private val data = ByteArray(200_000) { (it % 251).toByte() }
    private lateinit var pc: ServerSocket
    private val proxy = StreamProxy()
    private val seenAuth = ArrayList<String?>()

    @Before fun startPc() {
        // Fake PC share: Basic auth required, single byte ranges.
        pc = ServerSocket(0, 16, InetAddress.getLoopbackAddress())
        thread(isDaemon = true) {
            while (!pc.isClosed) {
                val c = try { pc.accept() } catch (_: Exception) { break }
                thread(isDaemon = true) {
                    c.use {
                        val head = StreamProxy.readHead(c.getInputStream()) ?: return@use
                        val hdr = head.split("\r\n").drop(1).mapNotNull { l ->
                            l.indexOf(':').takeIf { it > 0 }?.let { l.substring(0, it).lowercase() to l.substring(it + 1).trim() } }.toMap()
                        seenAuth += hdr["authorization"]
                        val out = c.getOutputStream()
                        if (hdr["authorization"] != "Basic secret") {
                            out.write("HTTP/1.1 401 No\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".toByteArray()); return@use
                        }
                        var s = 0; var e = data.size - 1; var code = "200 OK"; var cr = ""
                        hdr["range"]?.let { r ->
                            val (x, y) = r.removePrefix("bytes=").split("-")
                            s = x.toInt(); if (y.isNotEmpty()) e = y.toInt(); code = "206 Partial"
                            cr = "Content-Range: bytes $s-$e/${data.size}\r\n"
                        }
                        out.write(("HTTP/1.1 $code\r\nContent-Type: video/mp4\r\n$cr" +
                            "Content-Length: ${e - s + 1}\r\nConnection: close\r\n\r\n").toByteArray())
                        out.write(data, s, e - s + 1); out.flush()
                    }
                }
            }
        }
    }

    @After fun stop() { pc.close(); proxy.close() }

    private fun src(auth: String = "Basic secret") =
        StreamSource("http://127.0.0.1:${pc.localPort}/v.mp4", mapOf("Authorization" to auth), data.size.toLong(), "video/mp4")

    private fun open(url: String, range: String? = null, method: String = "GET") =
        (URL(url).openConnection() as HttpURLConnection).apply { requestMethod = method; range?.let { setRequestProperty("Range", it) } }

    @Test fun rangesPassThroughWithoutGivingThePinToThePlayer() {
        val url = proxy.add(src(), "My video.mp4")
        assertTrue(url.startsWith("http://127.0.0.1:") && url.endsWith("/My%20video.mp4"))
        val c = open(url, "bytes=1000-1999")
        assertEquals(206, c.responseCode)
        assertEquals("bytes 1000-1999/${data.size}", c.getHeaderField("Content-Range"))
        assertArrayEquals(data.copyOfRange(1000, 2000), c.inputStream.readBytes())
        assertEquals("Basic secret", seenAuth.last())
        val whole = open(url)
        assertEquals(200, whole.responseCode)
        assertArrayEquals(data, whole.inputStream.readBytes())
        val head = open(url, method = "HEAD")
        assertEquals(200, head.responseCode)
        assertEquals("${data.size}", head.getHeaderField("Content-Length"))
    }

    @Test fun unknownTokenWrongPinAndOtherMethods() {
        val url = proxy.add(src(), "a.mp4")
        val base = url.substringBeforeLast('/').substringBeforeLast('/')
        assertEquals(404, open("$base/nope/a.mp4").responseCode)
        assertEquals(405, open(url, method = "DELETE").responseCode)
        assertEquals(502, open(proxy.add(src("Basic wrong"), "b.mp4")).responseCode)
        repeat(StreamProxy.MAX_ITEMS) { proxy.add(src(), "c.mp4") }
        assertEquals(404, open(url).responseCode)          // oldest link dropped
    }

    @Test fun choiceForVideosOver5MbAndMusicOver50Mb() {
        assertTrue(wantsChoice("video/mp4", VIDEO_MIN))
        assertTrue(!wantsChoice("video/mp4", VIDEO_MIN - 1))
        assertTrue(!wantsChoice("audio/mpeg", 20L * 1024 * 1024))
        assertTrue(wantsChoice("audio/flac", AUDIO_MIN))
        assertTrue(!wantsChoice("application/pdf", 1L shl 30))
    }

    @Test fun onlySingleByteRangesArePassedOn() {
        assertEquals("bytes=5-", StreamProxy.parseHead("GET /t/a HTTP/1.1\r\nRange: bytes=5-\r\n\r\n")!!.range)
        assertNull(StreamProxy.parseHead("GET /t/a HTTP/1.1\r\nRange: bytes=0-1,4-5\r\n\r\n")!!.range)
        assertNull(StreamProxy.parseHead("GET /t/a HTTP/1.1\r\nRange: bytes=0-1\r\nX: y\r\nRange: evil\r\n\r\n")!!.range)
        assertNull(StreamProxy.parseHead("GARBAGE\r\n\r\n"))
    }
}
