package dev.shulhi.rex
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.*
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ConnectionChromeTest {
    @get:Rule val ui=createComposeRule()
    @Test fun theHeaderStatesWhatIsWrongInItsSecondLineWithOneAction() {
        val status=mutableStateOf<HeaderStatus?>(null);var retries=0;var takes=0
        ui.setContent {MaterialTheme {SessionHeader(listOf(SessionItem("a","alpha")),"a",true,true,emptyList(),0,"~/dev",{},{},{},title="fish",
            paneDots=1 to 3,status=status.value) {}}}
        ui.onNodeWithContentDescription("Session strip").assertIsDisplayed()
        ui.onNodeWithContentDescription("Pane 2 of 3").assertIsDisplayed()
        val height=ui.onRoot().fetchSemanticsNode().boundsInRoot.height
        ui.runOnIdle {status.value=HeaderStatus("Offline",StatusTone.WARNING,"Retry",enabled=false) {retries++}}
        ui.onNodeWithContentDescription("Session strip").assertDoesNotExist()
        ui.onNodeWithText("Offline").assertIsDisplayed()
        ui.onNode(hasText("Retry") and hasClickAction()).assertIsNotEnabled()
        // The status takes the session line's place, so the terminal below never moves.
        assertEquals(height,ui.onRoot().fetchSemanticsNode().boundsInRoot.height)
        ui.runOnIdle {status.value=HeaderStatus("Viewing only",StatusTone.QUIET,"Take over") {takes++}}
        ui.onNode(hasText("Take over") and hasClickAction()).performClick()
        ui.runOnIdle {assertEquals(1,takes);assertEquals(0,retries);status.value=null}
        ui.onNodeWithText("Take over").assertDoesNotExist()
        ui.onNodeWithContentDescription("Session strip").assertIsDisplayed()
    }
    @Test fun reconnectNoticeMakesPausedInputAndRetryExplicit() {
        val online=mutableStateOf<Boolean?>(false);var retries=0
        ui.setContent {MaterialTheme {ConnectionNotice("reconnecting",online.value,{retries++})}}
        ui.onNodeWithText("Waiting for network · input paused").assertIsDisplayed()
        ui.onNodeWithText("Retry now").assertIsNotEnabled()
        ui.runOnIdle {online.value=true}
        ui.onNodeWithText("Reconnecting · input paused").assertIsDisplayed()
        ui.onNodeWithText("Retry now").performClick();ui.runOnIdle {assertEquals(1,retries)}
    }
    @Test fun readyNoticeCancelsWhenInputStopsBeingReady() {
        val owned=mutableStateOf(true)
        ui.setContent {MaterialTheme {ReadyNotice(1,owned.value,true)}}
        ui.mainClock.autoAdvance=false;ui.mainClock.advanceTimeByFrame()
        ui.onNodeWithText("Ready to type").assertIsDisplayed()
        ui.runOnIdle {owned.value=false};ui.mainClock.advanceTimeBy(2000)
        ui.onNodeWithText("Ready to type").assertDoesNotExist()
    }
}
