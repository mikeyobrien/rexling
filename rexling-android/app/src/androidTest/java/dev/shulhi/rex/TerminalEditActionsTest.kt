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
class TerminalEditActionsTest {
    @get:Rule val ui=createComposeRule()
    private val fish=ProgramIdentity.resolve(listOf("fish"))
    private val claude=ProgramIdentity.resolve(listOf("claude"))
    private val codex=ProgramIdentity.resolve(listOf("codex"))
    private val tab=mutableStateOf(TabEntry("w","Project","",listOf(fish,claude,codex),0,false,false,"Project",
        listOf(PaneEntry("a","fish","","shell",fish),PaneEntry("b","Agent task","","shell",claude),PaneEntry("f","Floating task","","shell",codex,floating=true)),
        listOf(PanePlacement("a",0f,0f,.5f,1f),PanePlacement("b",.5f,0f,.5f,1f),PanePlacement("f",.1f,.1f,.7f,.7f,floating=true))))
    private val calls=mutableListOf<String>()
    private fun show() {
        ui.setContent {RexTheme(false,ThemeCatalog.defaults[1]) {
            Switcher(SwitcherModel("Mac","Session",listOf(tab.value),"w","a",openTab={calls+="open-tab"},openPane={calls+="open-pane"},
                renameTab={id,label->calls+="rename-tab:$id:$label";true},renamePane={window,id,label->calls+="rename-pane:$window:$id:$label";true},
                floatPane={window,id->
                    calls+="float:$window:$id"
                    tab.value=tab.value.copy(panes=tab.value.panes.map {if(it.id==id) it.copy(floating=true) else it},layout=tab.value.layout.map {if(it.id==id) it.copy(floating=true) else it});true
                },dockPane={window,id,anchor,split->calls+="dock:$window:$id:$anchor:${split.name}";true}),onClose={})
        }}
    }
    private fun hold(text: String) {ui.onNode(hasText(text,substring=true) and hasClickAction()).performTouchInput {longClick()};ui.waitForIdle()}
    @Test fun tabAndPaneRenameActionsNeverOpenTheTerminal() {
        show()
        hold("Project")
        ui.onNodeWithText("Rename").performClick()
        ui.onNodeWithText("Rename tab").assertIsDisplayed()
        ui.onNode(hasSetTextAction()).performTextReplacement("Release")
        ui.onNodeWithText("Save").performClick()
        hold("fish")
        ui.onNodeWithText("Rename").performClick()
        ui.onNodeWithText("Rename pane").assertIsDisplayed()
        ui.onNode(hasSetTextAction()).performTextReplacement("Console")
        ui.onNodeWithText("Save").performClick()
        ui.runOnIdle {assertEquals(listOf("rename-tab:w:Release","rename-pane:w:a:Console"),calls)}
    }
    @Test fun dockingChoosesTheTargetAndSideWhileFloatingKeepsOneDockedPane() {
        show()
        hold("Floating task")
        ui.onNodeWithText("Float").assertDoesNotExist()
        ui.onNodeWithText("Dock").performClick()
        ui.onNodeWithText("Split below").performClick()
        ui.onNode(hasText("Dock") and hasClickAction()).performClick()
        hold("Agent task")
        ui.onNodeWithText("Float").performClick()
        // One docked pane is left, and a tab keeps at least one.
        hold("fish")
        ui.onNodeWithText("Rename").assertIsDisplayed()
        ui.onNodeWithText("Float").assertDoesNotExist()
        ui.runOnIdle {assertEquals(listOf("dock:w:f:a:BELOW","float:w:b"),calls)}
    }
    @Test fun rejectedRenameKeepsTheInputAndSupportsResetToAutomatic() {
        val target=TerminalRenameTarget("w","a",false,"My pane");var accept=false;var closed=0;var value=""
        ui.setContent {RexTheme(true,ThemeCatalog.defaults[0]) {
            TerminalRenameDialog(target,true,{value=it;accept}) {closed++}
        }}
        ui.onNodeWithText("Save").performClick()
        ui.onNode(hasSetTextAction()).assertTextContains("My pane")
        ui.runOnIdle {assertEquals(0,closed);accept=true}
        ui.onNode(hasSetTextAction()).performTextClearance()
        ui.onNodeWithText("Save").performClick()
        ui.runOnIdle {assertEquals("",value);assertEquals(1,closed)}
    }
}
