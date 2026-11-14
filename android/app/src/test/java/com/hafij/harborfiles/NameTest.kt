package com.hafij.harborfiles

import org.junit.Assert.assertEquals
import org.junit.Test

class NameTest {
    @Test fun cleansDeviceName() {
        assertEquals("Redmi Pad 2", cleanName("  Redmi Pad 2 "))
        assertEquals("Pad Pro", cleanName("Pad (Pro)"))
        assertEquals("ab", cleanName("a\nb"))
        assertEquals(60, cleanName("x".repeat(90)).length)
        assertEquals("", cleanName("()"))
    }
}
