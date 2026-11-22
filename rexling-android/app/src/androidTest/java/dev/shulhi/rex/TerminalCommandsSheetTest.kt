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
            if(open) TerminalCommandsSheet("Shell",canCopy=true,catalog=null,serverRun=null,loadServerCommands={true},runServerCommand={_,_->false},onChoose={chosen=it;open=false},onClose={open=false},hazeState=null)
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

    @Test fun hostActionsAreSearchableAndExposeSuccessErrorsAndEmptyCatalogs() {
        val split=ServerCommand("split_project","Split at project root","Projects","workspace",false)
        val withArgs=ServerCommand("pick_project","Open named project","Projects","",true)
        var catalog by mutableStateOf(ServerCommandCatalog(emptyList(),""))
        var result by mutableStateOf<ServerCommandRun?>(null)
        var invoked=0
        ui.setContent {RexTheme(false,ThemeCatalog.defaults[1]) {
            TerminalCommandsSheet("Shell",false,catalog,result,{true},{name,id->
                assertEquals(split.name,name)
                invoked++
                result=if(invoked==1) ServerCommandRun(id,false,"","Directory not found","")
                else ServerCommandRun(id,false,"Project split created","", "")
                true
            },onChoose={},onClose={},hazeState=null)
        }}
        ui.onNodeWithText("No commands configured").assertExists()
        ui.runOnIdle {catalog=ServerCommandCatalog(emptyList(),"Host unavailable")}
        ui.onNodeWithText("Host unavailable").assertExists()
        ui.onNodeWithText("Retry").assertExists()
        ui.runOnIdle {catalog=ServerCommandCatalog(listOf(split,withArgs),"")}
        ui.onNodeWithText(withArgs.title).assertIsNotEnabled()
        ui.onNode(hasSetTextAction()).performTextInput("workspace")
        ui.onNodeWithText(withArgs.title).assertDoesNotExist()
        val action=ui.onNodeWithText(split.title)
        action.performClick()
        ui.onNodeWithText("Directory not found").assertIsDisplayed()
        action.performClick()
        ui.onNodeWithText("Project split created").assertIsDisplayed()
        ui.runOnIdle {assertEquals(2,invoked)}
    }
}
