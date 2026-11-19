package dev.shulhi.rex

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

class KeyRowWidthTest {
    @get:Rule val ui=createComposeRule()

    /** Hiding and showing the tab list resizes the row; its keys must keep filling it from the first to the last. */
    @Test fun theKeysRefillTheRowWhenItsWidthChanges() {
        val width=mutableStateOf(300.dp)
        ui.setContent {MaterialTheme {Box(Modifier.width(width.value)) {
            KeyboardBar(true,true,false,false,false,true,{},{},{},{},{},{},onKeyboard={})
        }}}
        fun edges()=ui.onNodeWithContentDescription("Terminal keys").fetchSemanticsNode().boundsInRoot to
            (ui.onNodeWithContentDescription("Escape").fetchSemanticsNode().boundsInRoot to ui.onNodeWithContentDescription("Arrow keys").fetchSemanticsNode().boundsInRoot)
        for(next in listOf(300.dp,360.dp,300.dp,340.dp)) {
            width.value=next;ui.waitForIdle()
            val (row,keys)=edges()
            assertEquals("Escape starts the row at $next",row.left,keys.first.left,1.5f)
            assertEquals("The arrows end the row at $next",row.right,keys.second.right,1.5f)
        }
    }
}
