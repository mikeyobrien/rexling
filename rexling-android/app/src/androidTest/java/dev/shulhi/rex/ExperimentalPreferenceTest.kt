package dev.shulhi.rex

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ExperimentalPreferenceTest {
    @get:Rule val ui=createComposeRule()
    @Test fun codexOptionTogglesIndependently() {
        val enabled=mutableStateOf(false);var calls=0
        ui.setContent {RexTheme(true,ThemeCatalog.defaults[0]) {
            PreferencesSheet({},codexTitleAttention=enabled.value,onCodexTitleAttention={enabled.value=it;calls++})
        }}
        val toggle=ui.onNode(hasText("Codex title attention") and isToggleable())
        toggle.performScrollTo().assertIsOff().performClick().assertIsOn()
        ui.runOnIdle {assertEquals(1,calls)}
        toggle.performClick().assertIsOff()
    }
}
