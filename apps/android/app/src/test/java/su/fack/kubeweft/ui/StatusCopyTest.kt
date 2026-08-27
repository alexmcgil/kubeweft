package su.fack.kubeweft.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class StatusCopyTest {
    @Test
    fun disconnectedStatusCopyIsStable() {
        assertEquals(
            StatusCopy(title = "Kubeweft", status = "No cluster connected"),
            statusCopy(),
        )
    }
}
