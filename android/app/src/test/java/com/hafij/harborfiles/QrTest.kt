package com.hafij.harborfiles

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class QrTest {
    @Test fun pcQrWithPin() {
        val q = parseConnectQr("http://192.168.1.5:8765/#pin=123456")!!
        assertEquals(ScannedServer("dav", "192.168.1.5", 8765, "harbor", "/dav/", "123456"), q)
    }

    @Test fun pcQrWithoutPin() {
        val q = parseConnectQr("http://192.168.1.5:8765/")!!
        assertEquals("dav", q.type); assertNull(q.secret)
    }

    @Test fun phoneQr() {
        val q = parseConnectQr("ftp://harbor:654321@10.0.0.7:2121/")!!
        assertEquals(ScannedServer("ftp", "10.0.0.7", 2121, "harbor", "", "654321"), q)
    }

    @Test fun phoneQrMatchesServerInfo() {
        val i = ServerInfo("10.0.0.7", 2121, "654321")
        assertEquals("654321", parseConnectQr(i.qrText)!!.secret)
    }

    @Test fun rejectsOtherCodes() {
        assertNull(parseConnectQr("WIFI:S:home;T:WPA;P:secret;;"))
        assertNull(parseConnectQr("hello"))
        assertNull(parseConnectQr("http://host:99999/"))
    }
}
