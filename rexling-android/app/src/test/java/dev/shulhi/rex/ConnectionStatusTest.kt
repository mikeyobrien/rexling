package dev.shulhi.rex

import org.junit.Assert.*
import org.junit.Test

class ConnectionStatusTest {
    @Test fun theHeaderNamesTheConnectionOnlyWhileInputIsPaused() {
        assertNull(connectionStatus("connected",true));assertNull(connectionStatus("preview",null));assertNull(connectionStatus("disconnected",false))
        assertEquals("Connecting…",connectionStatus("connecting",true))
        assertEquals("Reconnecting…",connectionStatus("reconnecting",null));assertEquals("Reconnecting…",connectionStatus("error",true))
        // Without a network there is nothing to retry against, whatever the phase.
        for(phase in listOf("connecting","reconnecting","error")) assertEquals("Offline",connectionStatus(phase,false))
    }
}
