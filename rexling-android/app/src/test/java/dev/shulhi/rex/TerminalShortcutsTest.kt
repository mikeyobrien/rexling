package dev.shulhi.rex

import android.view.KeyEvent
import org.junit.Assert.*
import org.junit.Test

class TerminalShortcutsTest {
    private fun resolve(keyCode: Int, ctrl: Boolean = true, alt: Boolean = true, shift: Boolean = false, meta: Boolean = false) =
        TerminalShortcuts.resolve(keyCode, ctrl, alt, shift, meta)

    @Test fun ctrlAltWalksPanesSessionsAndOpensThePickers() {
        assertEquals(TerminalShortcut.PreviousPane, resolve(KeyEvent.KEYCODE_DPAD_LEFT))
        assertEquals(TerminalShortcut.NextPane, resolve(KeyEvent.KEYCODE_DPAD_RIGHT))
        assertEquals(TerminalShortcut.PreviousSession, resolve(KeyEvent.KEYCODE_DPAD_UP))
        assertEquals(TerminalShortcut.NextSession, resolve(KeyEvent.KEYCODE_DPAD_DOWN))
        assertEquals(TerminalShortcut.NextPane, resolve(KeyEvent.KEYCODE_TAB))
        assertEquals(TerminalShortcut.PreviousPane, resolve(KeyEvent.KEYCODE_TAB, shift = true))
        assertEquals(TerminalShortcut.NewTab, resolve(KeyEvent.KEYCODE_N))
        assertEquals(TerminalShortcut.PanePicker, resolve(KeyEvent.KEYCODE_P))
        assertEquals(TerminalShortcut.SessionPicker, resolve(KeyEvent.KEYCODE_S))
        assertEquals(TerminalShortcut.Keyboard, resolve(KeyEvent.KEYCODE_K))
    }

    @Test fun anythingAProgramCouldHaveAskedForStaysInput() {
        for (key in listOf(KeyEvent.KEYCODE_R, KeyEvent.KEYCODE_A, KeyEvent.KEYCODE_LEFT_BRACKET, KeyEvent.KEYCODE_1, KeyEvent.KEYCODE_TAB, KeyEvent.KEYCODE_DPAD_RIGHT)) {
            assertNull("plain $key", resolve(key, ctrl = false, alt = false))
            assertNull("ctrl $key", resolve(key, ctrl = true, alt = false))
            assertNull("alt $key", resolve(key, ctrl = false, alt = true))
            assertNull("meta $key", resolve(key, meta = true))
        }
    }

    @Test fun incompleteCombinationsAndUnmappedKeysDoNothing() {
        assertNull(resolve(KeyEvent.KEYCODE_DPAD_LEFT, alt = false))
        assertNull(resolve(KeyEvent.KEYCODE_DPAD_LEFT, ctrl = false))
        assertNull(resolve(KeyEvent.KEYCODE_N, shift = true))
        assertNull(resolve(KeyEvent.KEYCODE_VOLUME_UP))
        assertNull(resolve(KeyEvent.KEYCODE_0))
    }
}
