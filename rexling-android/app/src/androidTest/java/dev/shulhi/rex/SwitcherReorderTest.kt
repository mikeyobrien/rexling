package dev.shulhi.rex

import androidx.compose.runtime.*
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.test.*
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.*
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class SwitcherReorderTest {
    @get:Rule val ui=createComposeRule()
    private val fish=ProgramIdentity.resolve(listOf("fish"))
    private fun tab(id: String,title: String,vararg panes: String)=TabEntry(id,title,"",panes.map {fish},0,false,false,title,panes.map {PaneEntry(it,"Pane ${it.uppercase()}","",it,fish)},
        panes.mapIndexed {i,p->PanePlacement(p,i/panes.size.toFloat(),0f,1f/panes.size,1f)})
    private val tabs=mutableStateOf(listOf(tab("w0","One","a","b","c"),tab("w1","Two","x"),tab("w2","Three","y")))
    private val ready=mutableStateOf(true)
    private val moves=mutableListOf<String>()
    private val actions=mutableListOf<String>()
    private fun reorder(ids: List<String>,id: String,before: String?)=moveTabBefore(ids,id,before)!!
    private fun show() {
        ui.setContent {RexTheme(false,ThemeCatalog.defaults[1]) {
            Switcher(SwitcherModel("Mac","Session",tabs.value,"w0","a",enabled=ready.value,ready=ready.value,
                openTab={actions+="tab:$it"},openPane={actions+="pane:$it"},closeTab={actions+="close-tab"},closePane={actions+="close-pane"},
                moveTab={id,before->
                    moves+="tab:$id:$before"
                    val map=tabs.value.associateBy {it.id}
                    tabs.value=reorder(tabs.value.map {it.id},id,before).map {map.getValue(it)};true
                },movePane={window,id,before->
                    moves+="pane:$window:$id:$before"
                    tabs.value=tabs.value.map {t->if(t.id!=window) t else {
                        val map=t.panes.associateBy {it.id}
                        t.copy(panes=reorder(t.panes.map {it.id},id,before).map {map.getValue(it)})
                    }};true
                }),onClose={actions+="dismiss"})
        }}
    }
    private val list get()=ui.onNodeWithContentDescription("Tabs")
    private fun row(text: String)=ui.onNode(hasText(text,substring=true) and hasClickAction())
    private fun point(text: String)=row(text).fetchSemanticsNode().boundsInRoot.center-list.fetchSemanticsNode().boundsInRoot.topLeft
    private fun begin(text: String) {
        val point=point(text)
        ui.mainClock.autoAdvance=false
        list.performTouchInput {down(point)}
        ui.mainClock.advanceTimeBy(700);ui.waitForIdle()
    }
    private fun finish() {
        list.performTouchInput {up()};ui.mainClock.advanceTimeBy(500);ui.waitForIdle()
        ui.mainClock.autoAdvance=true
    }
    @Test fun holdingThenDraggingATabReordersItOnceWithoutOpeningItOrLeavingTheMenu() {
        show()
        val end=point("Three")
        begin("One")
        ui.onNodeWithText("Rename").assertIsDisplayed()
        list.performTouchInput {moveTo(end+Offset(0f,10f),200)}
        ui.mainClock.advanceTimeBy(100)
        // Carrying the row means reordering: the menu steps aside and the tab's panes fold away.
        ui.onNodeWithText("Rename").assertDoesNotExist()
        row("Pane B").assertDoesNotExist()
        finish()
        ui.runOnIdle {assertEquals(listOf("w1","w2","w0"),tabs.value.map {it.id});assertEquals(listOf("tab:w0:null"),moves);assertTrue(actions.isEmpty())}
        ui.onNodeWithText("Rename").assertDoesNotExist()
        row("Pane B").assertIsDisplayed()
    }
    @Test fun draggingAPaneReordersItWithinItsTabAndKeepsTheSelection() {
        show()
        val layout=tabs.value.first().layout
        val end=point("Pane C")
        begin("Pane A")
        list.performTouchInput {moveTo(end+Offset(0f,10f),200)}
        ui.mainClock.advanceTimeBy(100);finish()
        ui.runOnIdle {
            assertEquals(listOf("b","c","a"),tabs.value.first().panes.map {it.id});assertSame(layout,tabs.value.first().layout)
            assertEquals(listOf("pane:w0:a:null"),moves);assertTrue(actions.isEmpty())
        }
        row("Pane A").assertIsSelected()
    }
    @Test fun holdingWithoutMovingOpensTheMenuAndACancelledDragSendsNothing() {
        show()
        begin("Two");finish()
        ui.onNodeWithText("Rename").assertIsDisplayed()
        // A tap anywhere else only puts the menu away.
        ui.onNodeWithText("New tab").performClick()
        ui.onNodeWithText("Rename").assertDoesNotExist()
        begin("Two")
        list.performTouchInput {moveBy(Offset(0f,100f));cancel()};ui.mainClock.advanceTimeBy(500)
        ui.runOnIdle {assertTrue(moves.isEmpty());assertTrue(actions.isEmpty());assertEquals("w0",tabs.value.first().id)}
    }
    @Test fun aListThatChangesDuringADragCancelsTheMove() {
        show()
        val end=point("Two")
        begin("One")
        list.performTouchInput {moveTo(end+Offset(0f,10f),200)}
        ui.mainClock.advanceTimeBy(100)
        ui.runOnIdle {tabs.value=tabs.value.dropLast(1)}
        ui.mainClock.advanceTimeByFrame()
        finish()
        ui.runOnIdle {assertTrue(moves.isEmpty());assertTrue(actions.isEmpty())}
    }
    @Test fun aBusyServerCannotBeReordered() {
        ready.value=false
        show()
        begin("One")
        ui.onNodeWithText("Rename").assertDoesNotExist()
        list.performTouchInput {moveBy(Offset(0f,-100f))};finish()
        ui.runOnIdle {assertTrue(moves.isEmpty());assertTrue(actions.isEmpty())}
    }
    @Test fun holdingARowAtTheEdgeScrollsToRowsOffScreen() {
        tabs.value=(0..13).map {tab("t$it","Tab number $it","p$it")}
        show()
        begin("Tab number 0")
        val bounds=list.fetchSemanticsNode().boundsInRoot
        list.performTouchInput {moveTo(Offset(bounds.width/2,bounds.height-2f),200)}
        ui.mainClock.advanceTimeBy(3000);ui.waitForIdle();finish()
        ui.runOnIdle {assertEquals("t0",tabs.value.last().id);assertEquals(listOf("tab:t0:null"),moves);assertTrue(actions.isEmpty())}
    }
    @Test fun rowsCanBeMovedWithoutDraggingAndStopAtTheEnds() {
        show()
        fun labels(text: String)=row(text).fetchSemanticsNode().config[SemanticsActions.CustomActions].map {it.label}
        assertEquals(listOf("Rename","Split right","Split below","Close","Move down"),labels("One"))
        assertEquals(listOf("Move up","Move down"),labels("Two").filter {it.startsWith("Move")})
        assertEquals(listOf("Move up"),labels("Three").filter {it.startsWith("Move")})
        assertEquals(listOf("Move down"),labels("Pane A").filter {it.startsWith("Move")})
        val down=row("Pane A").fetchSemanticsNode().config[SemanticsActions.CustomActions].first {it.label=="Move down"}
        ui.runOnIdle {assertTrue(down.action())}
        ui.runOnIdle {assertEquals(listOf("b","a","c"),tabs.value.first().panes.map {it.id});assertEquals(listOf("pane:w0:a:c"),moves);assertTrue(actions.isEmpty())}
    }
}
