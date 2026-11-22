package dev.shulhi.rex

import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createEmptyComposeRule
import androidx.test.core.app.ActivityScenario
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assume.assumeTrue
import org.junit.Rule
import org.junit.Test

class HostCommandsTest {
    @get:Rule val ui=createEmptyComposeRule()

    // Fixture actions use real host Lua. Never replace settings in the regular app.
    @Test fun terminalMenuDiscoversAndRunsActionsOnTheSelectedPane() {
        val args=InstrumentationRegistry.getArguments()
        val endpoint=args.getString("commands_endpoint")
        val session=args.getString("commands_session")
        val context=InstrumentationRegistry.getInstrumentation().targetContext
        assumeTrue(context.packageName.endsWith(".renderbench") && endpoint!=null && session!=null)
        val profile=ServerProfile.create("Commands fixture",endpoint!!,"commands-fixture")
        val prefs=context.getSharedPreferences("rex",0)
        prefs.edit().clear().putString("servers",SavedServers.encode(listOf(profile)))
            .putString("active_server",profile.id).putStringSet("connected_servers",setOf(profile.id))
            .putString("current_session:$endpoint",session).commit()
        try {
            ActivityScenario.launch(MainActivity::class.java).use {
                ui.waitUntil(20000) {ui.onAllNodesWithContentDescription("Terminal menu").fetchSemanticsNodes().isNotEmpty()}
                fun openCommands() {
                    ui.onNodeWithContentDescription("Terminal menu").performClick()
                    val commands=hasText("Commands") and isEnabled()
                    ui.waitUntil(10000) {ui.onAllNodes(commands).fetchSemanticsNodes().isNotEmpty()}
                    ui.onNode(commands).performClick()
                }
                openCommands()
                ui.onNodeWithText("Split pane…").assertExists()
                val failure=ui.onNodeWithText("Fail fixture command")
                val success=ui.onNodeWithText("Run fixture command")
                ui.waitUntil(10000) {ui.onAllNodesWithText("Run fixture command").fetchSemanticsNodes().isNotEmpty()}
                failure.performClick()
                ui.waitUntil(10000) {ui.onAllNodesWithText("FIXTURE_ACTION_FAILED",substring=true).fetchSemanticsNodes().isNotEmpty()}
                success.performClick()
                ui.waitUntil(10000) {ui.onAllNodesWithText("Fixture ran on the selected pane").fetchSemanticsNodes().isNotEmpty()}
                ui.onNodeWithText("Fixture ran on the selected pane").assertIsDisplayed()
                ui.onNodeWithText("Reveal fixture tab").performClick()
                ui.waitUntil(10000) {ui.onAllNodesWithContentDescription("Close commands").fetchSemanticsNodes().isEmpty()}
                openCommands()
                ui.waitUntil(10000) {ui.onAllNodesWithText("Check fixture tab").fetchSemanticsNodes().isNotEmpty()}
                ui.onNodeWithText("Check fixture tab").performClick()
                ui.waitUntil(10000) {ui.onAllNodesWithText("Rexling is viewing the requested tab").fetchSemanticsNodes().isNotEmpty()}
                ui.onNodeWithText("Rexling is viewing the requested tab").assertIsDisplayed()
            }
        } finally {prefs.edit().clear().commit()}
    }
}
