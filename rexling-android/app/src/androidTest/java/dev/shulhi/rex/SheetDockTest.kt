package dev.shulhi.rex

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class SheetDockTest {
    @get:Rule val ui=createComposeRule()
    private val dock=SheetDock()
    private var open by mutableStateOf(true)
    private var second by mutableStateOf(false)
    private var taps=0
    /** A list beside a terminal side, as the wide layout arranges them. */
    private fun wide(content: @Composable ()->Unit) {
        ui.setContent {MaterialTheme {CompositionLocalProvider(LocalSheetDock provides dock) {
            Row(Modifier.fillMaxSize()) {
                Box(Modifier.width(140.dp).testTag("list")) {Text("Tab list")}
                Box(Modifier.weight(1f).fillMaxHeight().testTag("side")) {
                    Text("Terminal")
                    SheetDockHost(dock)
                }
            }
            content()
        }}}
    }
    @Test fun aDockedSheetFillsTheTerminalSideAndLeavesTheListAlone() {
        dock.active=true
        wide {if(open) PreferencesSheet({open=false})}
        val side=ui.onNodeWithTag("side").fetchSemanticsNode().boundsInRoot
        val title=ui.onNodeWithText("Preferences").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
        assertTrue("The sheet belongs to the terminal side: $title in $side",title.left>=side.left && title.right<=side.right && title.top<side.top+80)
        ui.onNodeWithText("Tab list").assertIsDisplayed()
        ui.onNodeWithContentDescription("Close preferences").performClick()
        ui.onNodeWithText("Preferences").assertDoesNotExist()
        ui.runOnIdle {assertTrue(dock.entries.isEmpty())}
    }
    @Test fun theNewestDockedSheetIsOnTopAndBackClosesItFirst() {
        dock.active=true
        wide {
            if(open) PreferencesSheet({open=false},onAbout={second=true})
            if(second) AboutSheet {second=false}
        }
        ui.onNodeWithText("About").performScrollTo().performClick()
        ui.runOnIdle {assertEquals(2,dock.entries.size)}
        androidx.test.espresso.Espresso.pressBack()
        ui.runOnIdle {assertFalse(second);assertTrue(open);assertEquals(1,dock.entries.size)}
    }
    @Test fun aDockedCardSitsOnItsSideAndATapBesideItClosesIt() {
        dock.active=true
        wide {if(open) ActionCard(ActionCardModel("work","nvim","In control",true,"63 × 58",null,
            listOf(CardAction("Copy",ChromeIcon.COPY) {taps++}),emptyList(),preferences={}),onClose={open=false})}
        val side=ui.onNodeWithTag("side").fetchSemanticsNode().boundsInRoot
        val card=ui.onNodeWithContentDescription("Terminal actions").assertIsDisplayed().fetchSemanticsNode().boundsInRoot
        assertTrue("$card in $side",card.left>=side.left && card.right<=side.right)
        ui.onNodeWithTag("side").performTouchInput {click(topCenter+androidx.compose.ui.geometry.Offset(0f,20f))}
        ui.onNodeWithContentDescription("Terminal actions").assertDoesNotExist()
        ui.runOnIdle {assertEquals(0,taps)}
    }
    @Test fun withoutAWideLayoutTheSameSheetRisesFromTheBottom() {
        wide {if(open) PreferencesSheet({open=false})}
        ui.onNodeWithText("Preferences").assertIsDisplayed()
        ui.runOnIdle {assertTrue(dock.entries.isEmpty())}
    }
}
