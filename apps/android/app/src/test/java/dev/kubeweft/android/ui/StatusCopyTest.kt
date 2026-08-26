package dev.kubeweft.android.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class StatusCopyTest {
    @Test
    fun `returns the disconnected cluster status`() {
        val copy = statusCopy()

        assertEquals("Kubeweft", copy.title)
        assertEquals("No cluster connected", copy.status)
    }
}
