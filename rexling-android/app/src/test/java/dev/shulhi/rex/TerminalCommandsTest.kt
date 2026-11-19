package dev.shulhi.rex

import org.junit.Assert.*
import org.junit.Test

class TerminalCommandsTest {
    private val state=AppState(phase="connected",connectionEpoch=3,selectedSession="work",selectedBlock="shell",
        windows=listOf(WindowItem("tab","Work",listOf(BlockItem("shell","Shell"),BlockItem("logs","Logs")))))

    @Test fun anOpenPaletteNeverActsOnAReplacementPaneOrConnection() {
        val target=TerminalCommandTarget.capture(state,false)!!
        assertTrue(target.matches(state,false))
        assertFalse(target.matches(state,true))
        assertFalse(target.matches(state.copy(connectionEpoch=4),false))
        assertFalse(target.matches(state.copy(navigationId=7),false))
        assertFalse(target.matches(state.copy(selectedBlock="logs"),false))
        assertFalse(target.matches(state.copy(selectedSession="other"),false))
        assertFalse(target.matches(state.copy(windows=emptyList()),false))
        assertNull(TerminalCommandTarget.capture(state.copy(phase="reconnecting"),false))
        assertNull(TerminalCommandTarget.capture(state.copy(creating=true),false))
    }

    @Test fun searchMatchesNamesAndScopesWithoutExposingUnavailableActions() {
        assertEquals(TerminalCommand.entries,TerminalCommand.matching("",true))
        assertEquals(listOf(TerminalCommand.RENAME),TerminalCommand.matching("  TAB   rename ",true))
        assertEquals(listOf(TerminalCommand.SPLIT,TerminalCommand.COPY),TerminalCommand.matching("pane",true))
        assertTrue(TerminalCommand.matching("copy",false).isEmpty())
    }

    @Test fun commandsCanFillTheSpareToolbarSlotWithoutChangingDefaults() {
        val custom=ToolbarLayout.defaults+ToolbarKey.COMMANDS
        assertEquals(ToolbarLayout.MAX_KEYS,custom.size)
        assertEquals(custom,ToolbarLayout.decode(ToolbarLayout.encode(custom)))
        assertFalse(ToolbarKey.COMMANDS in ToolbarLayout.decode(null))
    }
}
