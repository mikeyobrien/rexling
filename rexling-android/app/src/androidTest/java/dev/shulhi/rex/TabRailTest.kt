package dev.shulhi.rex

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.height
import androidx.compose.material3.MaterialTheme
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TabRailTest {
    @get:Rule val ui=createComposeRule()
    private val shell=ProgramIdentity.resolve(listOf("fish"))
    private fun tab(id: String,attention: Boolean=false)=TabEntry(id,"Tab $id","",listOf(shell),0,attention,false)
    @Test fun theCollapsedListKeepsEveryTabATapAwayAndCanExpand() {
        val opened=mutableListOf<String>();var expanded=0
        ui.setContent {MaterialTheme {Box(Modifier.height(500.dp)) {
            TabRail(SwitcherModel(tabs=listOf(tab("a"),tab("b",attention=true),tab("c")),currentTab="a",openTab={opened+=it})) {expanded++}
        }}}
        ui.onNodeWithContentDescription("Tab a").assertIsSelected()
        ui.onNodeWithContentDescription("Tab b").assertIsNotSelected().performClick()
        ui.onNodeWithContentDescription("Show tab list").performClick()
        ui.runOnIdle {assertEquals(listOf("b"),opened);assertEquals(1,expanded)}
    }
}
