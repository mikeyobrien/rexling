package dev.shulhi.rex

import androidx.compose.runtime.*
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test

class TerminalCommandsSheetTest {
    @get:Rule val ui=createComposeRule()

    @Test fun commandsKeyOpensTheSheetWithoutTypingAndSearchNarrowsTheList() {
        var sent=0
        var chosen: TerminalCommand?=null
        ui.setContent {RexTheme(false,ThemeCatalog.defaults[1]) {
            var open by remember {mutableStateOf(false)}
            KeyboardBar(false,true,false,false,false,true,{},{},{},{},{},{sent++},
                toolbarKeys=ToolbarLayout.defaults+ToolbarKey.COMMANDS,onCommands={open=true},onKeyboard={})
            if(open) TerminalCommandsSheet("Shell",canCopy=true,onChoose={chosen=it;open=false},onClose={open=false},hazeState=null)
        }}
        ui.onNodeWithContentDescription("Commands").performClick()
        ui.onNodeWithText("Split pane…").assertIsDisplayed()
        val search=ui.onNode(hasSetTextAction())
        search.assertIsNotFocused()
        search.performTextInput("rename")
        ui.onNodeWithText("Copy viewport").assertDoesNotExist()
        ui.onNodeWithText("Rename tab…").performClick()
        ui.onNodeWithContentDescription("Close commands").assertDoesNotExist()
        ui.runOnIdle {assertEquals(TerminalCommand.RENAME,chosen);assertEquals(0,sent)}
    }
}
