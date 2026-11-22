package com.hafij.harborfiles

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.DataInputStream
import java.io.PipedInputStream
import java.io.PipedOutputStream
import java.net.Socket
import java.nio.ByteBuffer
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

class UsbLinkTest {
    @Test fun padNeverEndsOnPacketBoundary() {
        for (n in 0..300) assertNotEquals(0, UsbLink.pad(ByteArray(n)).size % 64)
    }

    @Test fun parserReassemblesAnySplit() {
        val big = ByteArray(UsbLink.MAX_PAYLOAD) { (it % 251).toByte() }
        val raw = UsbLink.pad(UsbLink.frame(UsbLink.DATA, 3, big)) + UsbLink.frame(UsbLink.CLOSE, 3)
        for (step in listOf(1, 7, 512, 16384)) {
            val p = UsbLink.FrameParser()
            val got = ArrayList<Triple<Int, Int, ByteArray>>()
            var i = 0
            while (i < raw.size) {
                val n = minOf(step, raw.size - i)
                got += p.feed(raw.copyOfRange(i, i + n), n); i += n
            }
            assertEquals(2, got.size)
            assertTrue(got[0].third.contentEquals(big))
            assertEquals(UsbLink.CLOSE, got[1].first)
        }
    }

    /** A fake PC on the other end of two pipes: HELLO, then echo one request. */
    @Test fun helloThenRoundTripThroughLoopback() {
        val toPhone = PipedOutputStream(); val phoneIn = PipedInputStream(toPhone, 1 shl 20)
        val phoneOut = PipedOutputStream(); val fromPhone = PipedInputStream(phoneOut, 1 shl 20)
        val pc = AtomicReference<UsbPc>()
        val ready = CountDownLatch(1)
        val link = UsbLink(phoneIn, phoneOut, { pc.set(it); ready.countDown() }, {})
        link.start()
        toPhone.write(UsbLink.frame(UsbLink.HELLO, 0, "v=1\nname=desk\npin=123456\nport=8765".toByteArray()))
        toPhone.flush()
        assertTrue(ready.await(5, TimeUnit.SECONDS))
        assertEquals("desk", pc.get().name)
        assertEquals("123456", pc.get().pin)

        val d = DataInputStream(fromPhone)
        fun next(): Triple<Int, Int, ByteArray> {
            while (true) {
                val k = d.readUnsignedByte(); val id = d.readInt(); val n = d.readInt()
                val p = ByteArray(n).also { d.readFully(it) }
                if (k != UsbLink.NOP && k != UsbLink.HELLO) return Triple(k, id, p)
            }
        }
        val c = Socket("127.0.0.1", pc.get().port)
        c.getOutputStream().write("ping".toByteArray()); c.getOutputStream().flush()
        val open = next()
        assertEquals(UsbLink.OPEN, open.first)
        val data = next()
        assertEquals(UsbLink.DATA, data.first)
        assertEquals("ping", String(data.third))
        toPhone.write(UsbLink.pad(UsbLink.frame(UsbLink.DATA, open.second, "pong".toByteArray())))
        toPhone.write(UsbLink.pad(UsbLink.frame(UsbLink.CLOSE, open.second)))
        toPhone.flush()
        val got = c.getInputStream().readBytes()
        assertEquals("pong", String(got))
        val ack = next()
        assertEquals(UsbLink.ACK, ack.first)
        assertEquals(4, ByteBuffer.wrap(ack.third).int)
        link.stop()
    }
}
