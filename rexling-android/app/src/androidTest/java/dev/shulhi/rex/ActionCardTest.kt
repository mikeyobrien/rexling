package dev.shulhi.rex

import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.*
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ActionCardTest {
    @get:Rule val ui=createComposeRule()
    private val calls=mutableListOf<String>()
    private fun card(controlled: Boolean=true,status: String=if(controlled) "In control" else "Viewing only",servers: String="") {
        ui.setContent {
            var open by remember {mutableStateOf(true)}
            MaterialTheme {
                if(open) ActionCard(ActionCardModel("work · Mac","nvim",status,controlled,"63 × 58",
                    CardAction(if(controlled) "Release" else "Take over",ChromeIcon.POINTER) {calls+="ownership"},
                    listOf(CardAction("Paste",ChromeIcon.PASTE,run=if(controlled) ({calls+="paste"}) else null),
                        CardAction("Image",ChromeIcon.IMAGE,run=if(controlled) ({calls+="image"}) else null),
                        CardAction("Copy",ChromeIcon.COPY) {calls+="copy"},CardAction("Keyboard",ChromeIcon.KEYBOARD) {calls+="keyboard"}),
                    listOf(CardAction("New session",ChromeIcon.ADD) {calls+="session"},CardAction("Port forwarding",ChromeIcon.SERVER) {calls+="forwards"},
                        CardAction("Servers",ChromeIcon.SERVER,servers) {calls+="servers"}),
                    preferences={calls+="preferences"}),onClose={calls+="close";open=false})
            }
        }
    }

    @Test fun itNamesThePaneAndStatesItsStatusAsChips() {
        card()
        ui.onNodeWithText("work · Mac").assertIsDisplayed();ui.onNodeWithText("nvim").assertIsDisplayed()
        ui.onNodeWithContentDescription("Status").assert(hasText("In control"))
        ui.onNodeWithContentDescription("Terminal size 63 × 58").assertIsDisplayed()
        ui.onNodeWithText("Release").assertIsDisplayed().assertHasClickAction()
    }
    @Test fun anActionLeavesTheCardBeforeItRuns() {
        card();ui.onNodeWithText("Paste").performClick()
        ui.runOnIdle {assertEquals(listOf("close","paste"),calls)}
        ui.onNodeWithContentDescription("Terminal actions").assertDoesNotExist()
    }
    @Test fun aPlaceRowRunsItsOwnAction() {
        card();ui.onNodeWithText("Port forwarding").performClick()
        ui.runOnIdle {assertEquals(listOf("close","forwards"),calls)}
    }
    @Test fun preferencesAndCloseSitInTheHeader() {
        card();ui.onNodeWithContentDescription("Preferences").performClick()
        ui.runOnIdle {assertEquals(listOf("close","preferences"),calls)}
    }
    @Test fun closingRunsNothingElse() {
        card();ui.onNodeWithContentDescription("Close actions").performClick()
        ui.runOnIdle {assertEquals(listOf("close"),calls)}
    }
    @Test fun whatNeedsControlIsDisabledWhileViewingAndTheOwnershipActionOffersToTakeOver() {
        card(controlled=false)
        ui.onNodeWithText("Viewing only").assertIsDisplayed()
        ui.onNodeWithText("Paste").assertIsNotEnabled();ui.onNodeWithText("Image").assertIsNotEnabled()
        ui.onNodeWithText("Copy").assertIsEnabled()
        ui.onNodeWithText("Take over").performClick()
        ui.runOnIdle {assertEquals(listOf("close","ownership"),calls)}
    }
    @Test fun aRowShowsItsDetailOnlyWhenThereIsOne() {
        card(servers="2 connected")
        ui.onNodeWithText("2 connected").assertIsDisplayed()
        ui.onNodeWithText("Servers").performClick()
        ui.runOnIdle {assertEquals(listOf("close","servers"),calls)}
    }
}
