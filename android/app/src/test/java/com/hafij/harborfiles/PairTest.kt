package com.hafij.harborfiles

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class PairTest {
    @Test fun parsesPairCode() {
        assertEquals(PairCode("192.168.1.5", 40123, "AbC_dEf-123456"),
            parsePairQr("harborpair://192.168.1.5:40123/AbC_dEf-123456"))
        assertEquals(PairCode("[fe80::1]", 9, "abcdefgh"), parsePairQr(" harborpair://[fe80::1]:9/abcdefgh/ "))
    }

    @Test fun rejectsOthers() {
        assertNull(parsePairQr("ftp://harbor:1@1.2.3.4:2121/"))
        assertNull(parsePairQr("harborpair://1.2.3.4:0/abcdefgh"))
        assertNull(parsePairQr("harborpair://1.2.3.4:80/short"))
        assertNull(parsePairQr("harborpair://1.2.3.4:80/abcdefgh/../x"))
        assertNull(parsePairQr("http://1.2.3.4:80/pair"))
    }
}
