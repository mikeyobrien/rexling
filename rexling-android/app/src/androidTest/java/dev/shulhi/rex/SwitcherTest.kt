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
class SwitcherTest {
    @get:Rule val ui=createComposeRule()
    private val fish=ProgramIdentity.resolve(listOf("fish"))
    private val claude=ProgramIdentity.resolve(listOf("claude"))
    private val codex=ProgramIdentity.resolve(listOf("codex"))
    private val nvim=ProgramIdentity.resolve(listOf("nvim"))
    private val project=TabEntry("w1","Project","3 panes · ~/Dev/portside",listOf(fish,claude,nvim),1,false,false,"Project",
        listOf(PaneEntry("a","fish","~/Dev/rexling","Terminal",fish),PaneEntry("b","Plan next steps","~/Dev/portside","Claude",claude),PaneEntry("d","nvim","","nvim",nvim)),
        listOf(PanePlacement("a",0f,0f,.4f,1f),PanePlacement("b",.4f,0f,.6f,.5f),PanePlacement("d",.4f,.5f,.6f,.5f)))
    private val review=TabEntry("w2","Review changes","",listOf(codex),0,true,false,"",listOf(PaneEntry("c","Review changes","","Codex",codex)))
    private val shell=TabEntry("w3","Scratch","~/Dev",listOf(fish),0,false,true,"",listOf(PaneEntry("e","fish","~/Dev","shell",fish)))
    private val tabs=mutableStateOf(listOf(project,review,shell))
    private val mac=ServerProfile.create("Shulhi MBP","https://fixture.example","mac")
    private val linux=ServerProfile.create("Ubuntu","https://linux.example","linux")
    private val here=SessionEntry(SessionTarget(mac,SessionItem("s","Rexling development"),SessionDetail("this device",3,5)),working=true)
    private val other=SessionEntry(SessionTarget(mac,SessionItem("other","ShipPDF"),SessionDetail("attached on macOS",2,2)),attention=true)
    private val remote=SessionEntry(SessionTarget(linux,SessionItem("s","Ubuntu server"),SessionDetail("this device",1,1)))
    private val sessions=mutableStateOf(listOf(here,other))
    private val calls=mutableListOf<String>()
    private fun show(ready: Boolean=true,dark: Boolean=false,sessionsFirst: Boolean=false) {
        ui.setContent {RexTheme(dark,ThemeCatalog.defaults[if(dark) 0 else 1]) {
            Switcher(SwitcherModel("Shulhi MBP","Rexling development",tabs.value,"w1","b",sessions.value,here.key,enabled=ready,ready=ready,canCreate=ready,canSplit=ready,
                openTab={calls+="tab:$it"},openPane={calls+="pane:$it"},newTab={calls+="new-tab"},split={anchor,split->calls+="split:$anchor:${split.direction}"},
                closeTab={calls+="close-tab:${it.id}"},closePane={calls+="close-pane:${it.id}"},
                openSession={calls+="session:$it"},newSession={calls+="new-session"},refreshSessions={calls+="refresh"},
                closeSession={calls+="close-session:${it.session.id}"},renameSession={target,label->calls+="rename-session:${target.session.id}:$label";true}),
                onClose={calls+="dismiss"},sessionsFirst=sessionsFirst)
        }}
    }
    private fun row(text: String)=ui.onNode(hasText(text,substring=true) and hasClickAction())
    private fun hold(text: String) {row(text).performTouchInput {longClick()};ui.waitForIdle()}

    @Test fun tappingATabOpensItAtOnce() {
        show()
        row("Review changes").performClick()
        ui.runOnIdle {assertEquals(listOf("tab:w2"),calls)}
    }
    @Test fun theCardOpensOnThePanesOfTheTabYouAreInAndMarksTheOneYouView() {
        show(dark=true)
        row("Plan next steps").assertIsSelected()
        row("~/Dev/rexling").assertIsNotSelected()
        row("Project").assertIsNotSelected()
        row("~/Dev/rexling").performClick()
        ui.runOnIdle {assertEquals(listOf("pane:a"),calls)}
        ui.onNodeWithContentDescription("Hide panes").performClick()
        row("Plan next steps").assertDoesNotExist()
        row("Project").assertIsSelected()
        ui.onNodeWithContentDescription("Show panes").performClick()
        row("Plan next steps").assertIsSelected()
        ui.runOnIdle {assertEquals(1,calls.size)}
    }
    @Test fun newTabAndTheSplitButtonsActOnThePaneYouView() {
        show()
        ui.onNodeWithText("New tab").performClick()
        ui.onNodeWithContentDescription("Split right").performClick()
        ui.onNodeWithContentDescription("Split below").performClick()
        ui.runOnIdle {assertEquals(listOf("new-tab","split:b:horizontal","split:b:vertical"),calls)}
    }
    @Test fun holdingARowOffersItsActionsWithoutOpeningIt() {
        show()
        ui.onNodeWithText("Rename").assertDoesNotExist()
        hold("Review changes")
        for(label in listOf("Rename","Split right","Split below","Close")) ui.onNodeWithText(label).assertIsDisplayed()
        ui.onNodeWithText("Split below").performClick()
        ui.onNodeWithText("Rename").assertDoesNotExist()
        ui.runOnIdle {assertEquals(listOf("split:c:vertical"),calls)}
        // A pane's own menu splits that pane, and can float it while another stays docked.
        hold("~/Dev/rexling")
        ui.onNodeWithText("Float").assertIsDisplayed()
        ui.onNodeWithText("Split right").performClick()
        ui.runOnIdle {assertEquals(listOf("split:c:vertical","split:a:horizontal"),calls)}
    }
    @Test fun aTapOutsideTheMenuOnlyDismissesIt() {
        show()
        hold("Review changes")
        ui.onNodeWithText("New tab").performClick()
        ui.onNodeWithText("Rename").assertDoesNotExist()
        ui.runOnIdle {assertTrue(calls.isEmpty())}
    }
    @Test fun closingARunningProgramAsksInTheRowAndAnIdleShellJustCloses() {
        show(dark=true)
        hold("Review changes")
        ui.onNodeWithText("Close").performClick()
        ui.onNodeWithText("Close Review changes?").assertIsDisplayed()
        ui.onNodeWithText("ends codex").assertIsDisplayed()
        ui.runOnIdle {assertTrue(calls.isEmpty())}
        ui.onNodeWithText("Cancel").performClick()
        ui.onNodeWithText("Close Review changes?").assertDoesNotExist()
        hold("Review changes")
        ui.onNodeWithText("Close").performClick()
        ui.onNodeWithText("Close").performClick()
        ui.runOnIdle {assertEquals(listOf("close-tab:w2"),calls)}
        hold("Scratch")
        ui.onNodeWithText("Close").performClick()
        ui.onNodeWithText("Close Scratch?").assertDoesNotExist()
        ui.runOnIdle {assertEquals(listOf("close-tab:w2","close-tab:w3"),calls)}
    }
    @Test fun aSidewaysSwipeShowsCloseAndATapPutsTheRowBack() {
        show()
        ui.onNodeWithContentDescription("Close Scratch").assertDoesNotExist()
        row("Scratch").performTouchInput {swipeLeft()}
        ui.onNodeWithContentDescription("Close Scratch").assertIsDisplayed()
        row("Scratch").performClick()
        ui.onNodeWithContentDescription("Close Scratch").assertDoesNotExist()
        ui.runOnIdle {assertTrue(calls.isEmpty())}
        row("Scratch").performTouchInput {swipeRight()}
        ui.onNodeWithContentDescription("Close Scratch").performClick()
        // Only one row is open at a time, and a pane closes on its own.
        row("~/Dev/rexling").performTouchInput {swipeLeft()}
        ui.onNodeWithContentDescription("Close fish").performClick()
        ui.runOnIdle {assertEquals(listOf("close-tab:w3","close-pane:a"),calls)}
    }
    @Test fun theHeadingGoesUpToSessionsAndTheSessionYouAreInComesBackDown() {
        show()
        ui.onNodeWithContentDescription("Sessions").performClick()
        ui.onNodeWithText("Sessions").assertIsDisplayed()
        row("ShipPDF").assertIsNotSelected()
        row("Rexling development").assertIsSelected()
        ui.onNodeWithText("3 tabs · 5 panes").assertIsDisplayed()
        ui.onNodeWithText("2 tabs").assertIsDisplayed()
        row("Rexling development").performClick()
        ui.onNodeWithText("New tab").assertIsDisplayed()
        ui.onNodeWithContentDescription("Sessions").performClick()
        row("ShipPDF").performClick()
        ui.runOnIdle {assertEquals(listOf("refresh","refresh","session:${other.key}"),calls)}
    }
    @Test fun sessionsCanOpenFirstAndNameTheirServersOnceThereAreSeveral() {
        sessions.value=listOf(here,other,remote)
        show(sessionsFirst=true,dark=true)
        ui.onNodeWithText("Ubuntu").assertIsDisplayed()
        ui.onNodeWithText("Shulhi MBP").assertIsDisplayed()
        ui.onNodeWithText("New session").performClick()
        row("Ubuntu server").performClick()
        ui.runOnIdle {assertEquals(listOf("refresh","new-session","session:${remote.key}"),calls)}
    }
    @Test fun aSessionIsRenamedAndClosedFromItsRowAndClosingAlwaysAsks() {
        show(sessionsFirst=true)
        hold("ShipPDF")
        ui.onNodeWithText("Split right").assertDoesNotExist()
        ui.onNodeWithText("Rename").performClick()
        ui.onNode(hasSetTextAction() and hasText("ShipPDF")).performTextReplacement("New project")
        ui.onNodeWithText("Save").performClick()
        hold("ShipPDF")
        ui.onNodeWithText("Close").performClick()
        ui.onNodeWithText("Close ShipPDF?").assertIsDisplayed()
        ui.onNodeWithText("ends 2 tabs").assertIsDisplayed()
        ui.runOnIdle {assertEquals(listOf("refresh","rename-session:other:New project"),calls)}
        ui.onNodeWithText("Close").performClick()
        ui.runOnIdle {assertEquals(listOf("refresh","rename-session:other:New project","close-session:other"),calls)}
    }
    @Test fun searchAppearsOnceSessionsNoLongerFitAndFiltersWithoutResizing() {
        show(sessionsFirst=true)
        ui.onNode(hasSetTextAction()).assertDoesNotExist()
        ui.runOnIdle {sessions.value=listOf(here,other,remote)+(1..5).map {SessionEntry(SessionTarget(mac,SessionItem("x$it","Extra $it")))}}
        val list=ui.onNodeWithContentDescription("Sessions list")
        // The keyboard takes its room once, when the field is focused; filtering must not move things again.
        ui.onNode(hasSetTextAction()).performClick();ui.waitForIdle();Thread.sleep(600)
        val before=list.fetchSemanticsNode().boundsInRoot
        ui.onNode(hasSetTextAction()).performTextInput("Ubuntu")
        row("Rexling development").assertDoesNotExist()
        assertEquals(before.height,list.fetchSemanticsNode().boundsInRoot.height)
        row("Ubuntu server").performClick()
        ui.runOnIdle {assertEquals(listOf("refresh","session:${remote.key}"),calls)}
        ui.onNode(hasSetTextAction()).performTextReplacement("nothing like this")
        ui.onNodeWithText("No matching sessions").assertIsDisplayed()
    }
    @Test fun aSessionWithoutTabsShowsSessionsAndAnEmptyServerStillOffersANewOne() {
        tabs.value=emptyList();sessions.value=emptyList()
        show()
        ui.onNodeWithText("No sessions yet").assertIsDisplayed()
        ui.onNodeWithText("New tab").assertDoesNotExist()
        ui.onNodeWithText("New session").performClick()
        ui.runOnIdle {assertEquals(listOf("refresh","new-session"),calls)}
    }
    @Test fun whileTheServerIsBusyRowsStayReadableButNothingChanges() {
        show(ready=false)
        row("Review changes").assertIsNotEnabled()
        ui.onNode(hasText("New tab") and hasClickAction()).assertIsNotEnabled()
        ui.onNodeWithContentDescription("Split right").assertIsNotEnabled()
        hold("Review changes")
        ui.onNodeWithText("Rename").assertDoesNotExist()
        ui.onNodeWithContentDescription("Close switcher").performClick()
        ui.runOnIdle {assertEquals(listOf("dismiss"),calls)}
    }
    @Test fun aRowTheServerRemovesTakesItsQuestionWithIt() {
        show()
        hold("Review changes")
        ui.onNodeWithText("Close").performClick()
        ui.onNodeWithText("Close Review changes?").assertIsDisplayed()
        ui.runOnIdle {tabs.value=listOf(project,shell)}
        ui.onNodeWithText("Close Review changes?").assertDoesNotExist()
        row("Scratch").assertIsDisplayed()
        ui.runOnIdle {assertTrue(calls.isEmpty())}
    }
}
