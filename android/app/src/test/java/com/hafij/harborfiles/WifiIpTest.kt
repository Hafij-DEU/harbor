package com.hafij.harborfiles

import java.net.InetAddress
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class WifiIpTest {
    private fun ip(s: String) = InetAddress.getByName(s)

    @Test fun vpnTunnelIsSkipped() {
        // Proton VPN on: the VPN network also says "Wi-Fi" and is listed first.
        val c = listOf(
            IpCandidate("tun0", true, listOf(ip("10.2.0.2"))),
            IpCandidate("wlan0", false, listOf(ip("fe80::1"), ip("192.168.1.102"))),
        )
        assertEquals("192.168.1.102", chooseWifiIp(c))
    }

    @Test fun tunnelNameWithoutVpnFlagIsSkipped() {
        assertEquals("192.168.1.5", chooseWifiIp(listOf(
            IpCandidate("tun0", false, listOf(ip("10.2.0.2"))),
            IpCandidate("wlan0", false, listOf(ip("192.168.1.5"))))))
    }

    @Test fun noUsableAddress() {
        assertNull(chooseWifiIp(listOf(IpCandidate("wlan0", false, listOf(ip("169.254.3.4"), ip("127.0.0.1"))))))
        assertNull(chooseWifiIp(emptyList()))
    }
}
