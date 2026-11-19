package dev.shulhi.rex


import org.junit.Assert.*
import org.junit.Test

class MotionTest {
    @Test fun theKeyRowFollowsTheKeyboardThroughItsAnimation() {
        assertEquals(0f,keyboardProgress(0,0,900),0f)
        assertEquals(.5f,keyboardProgress(450,0,900),.001f)
        assertEquals(1f,keyboardProgress(900,0,900),0f)
        // Closing: the target is zero, so the distance is where it started.
        assertEquals(.25f,keyboardProgress(225,900,0),.001f)
        // At rest, source and target agree with where it is.
        assertEquals(1f,keyboardProgress(900,900,900),0f);assertEquals(0f,keyboardProgress(0,0,0),0f)
        // Without animation data, shown is shown.
        assertEquals(1f,keyboardProgress(900,0,0),0f)
    }
    @Test fun rowsArriveInOrderOnlyWhileTheirCardIsOpening() {
        val rows=RowsState()
        assertNull(rows.arrival("tab:a"))
        rows.arriving=true
        assertEquals(listOf(0,1,0),listOf(rows.arrival("tab:a"),rows.arrival("tab:b"),rows.arrival("tab:a")))
        rows.arriving=false
        assertNull(rows.arrival("tab:c"))
    }
}
