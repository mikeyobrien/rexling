package dev.shulhi.rex

import android.view.KeyEvent

/** What a hardware keyboard can do that the touch UI does with buttons and swipes. */
internal enum class TerminalShortcut { PreviousPane, NextPane, PreviousSession, NextSession, NewTab, PanePicker, SessionPicker, Keyboard }

/**
 * Ctrl+Alt is the only modifier pair that cannot collide with the terminal: programs receive
 * plain keys, Ctrl+letter and Alt+letter (Meta) as control and escape sequences, and Android
 * reserves Meta. Directional pane focus needs `session.focus_direction`, which the client has
 * not wired yet, so left/right walk the pane cycle instead.
 */
internal object TerminalShortcuts {
    fun resolve(keyCode: Int, ctrl: Boolean, alt: Boolean, shift: Boolean, meta: Boolean): TerminalShortcut? {
        if (!ctrl || !alt || meta) return null
        if (shift && keyCode != KeyEvent.KEYCODE_TAB) return null
        return when (keyCode) {
            KeyEvent.KEYCODE_DPAD_LEFT -> TerminalShortcut.PreviousPane
            KeyEvent.KEYCODE_DPAD_RIGHT -> TerminalShortcut.NextPane
            KeyEvent.KEYCODE_DPAD_UP -> TerminalShortcut.PreviousSession
            KeyEvent.KEYCODE_DPAD_DOWN -> TerminalShortcut.NextSession
            KeyEvent.KEYCODE_TAB -> if (shift) TerminalShortcut.PreviousPane else TerminalShortcut.NextPane
            KeyEvent.KEYCODE_N -> TerminalShortcut.NewTab
            KeyEvent.KEYCODE_P -> TerminalShortcut.PanePicker
            KeyEvent.KEYCODE_S -> TerminalShortcut.SessionPicker
            KeyEvent.KEYCODE_K -> TerminalShortcut.Keyboard
            else -> null
        }
    }
}
