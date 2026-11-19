package dev.shulhi.rex

import org.junit.Assert.*
import org.junit.Test

class SwitcherModelTest {
    private val fish=ProgramIdentity.resolve(listOf("fish"))
    private val cargo=ProgramIdentity.resolve(listOf("fish","cargo"))
    private val nvim=ProgramIdentity.resolve(listOf("nvim"))
    private val server=ServerProfile.create("Mac","https://mac.example","mac")
    private val calls=mutableListOf<String>()
    private fun pane(id: String,program: ProgramIdentity=fish,floating: Boolean=false)=PaneEntry(id,id,"",id,program,floating)
    private fun tab(id: String,vararg panes: PaneEntry,shown: Int=0,layout: Boolean=true)=TabEntry(id,"Tab $id","",panes.map {it.program},shown,false,false,"label-$id",
        panes.toList(),if(layout) panes.mapIndexed {i,p->PanePlacement(p.id,i/panes.size.toFloat(),0f,1f/panes.size,1f,floating=p.floating)} else emptyList())
    private fun session(id: String,tabs: Int?=2)=SessionEntry(SessionTarget(server,SessionItem(id,"Session $id"),tabs?.let {SessionDetail("detached",it,it)}))
    private val tabs=listOf(tab("w1",pane("a"),pane("b",cargo),shown=1),tab("w2",pane("c")),tab("w3",pane("d",nvim)))
    private val sessions=listOf(session("s1"),session("s2",null))
    private fun model(tabs: List<TabEntry> = this.tabs,ready: Boolean=true,busy: String="")=SwitcherModel(tabs=tabs,sessions=sessions,ready=ready,busySession=busy,
        split={anchor,split->calls+="split:$anchor:${split.name}"},closeTab={calls+="close-tab:${it.id}"},closePane={calls+="close-pane:${it.id}"},
        moveTab={id,before->calls+="move-tab:$id:$before";true},movePane={tab,id,before->calls+="move-pane:$tab:$id:$before";true},
        floatPane={tab,id->calls+="float:$tab:$id";true},closeSession={calls+="close-session:${it.session.id}"})
    private fun menu(manager: RowManager,key: String)=manager.actions(key).filter {it.menu}.map {it.label}
    private fun run(manager: RowManager,key: String,label: String)=manager.actions(key).first {it.label==label}.run()

    @Test fun idleShellsCloseWithoutAskingAndRunningProgramsAreNamed() {
        assertEquals("",endsLine(listOf(fish,fish)))
        assertEquals("ends cargo",endsLine(listOf(fish,cargo)))
        assertEquals("ends nvim, cargo",endsLine(listOf(nvim,cargo,cargo)))
        assertEquals("ends 3 programs",endsLine(listOf(nvim,cargo,ProgramIdentity.resolve(listOf("htop")))))
    }
    @Test fun aPaneThatIsNotIdentifiedCouldRunAnythingSoItAsks() {
        assertEquals("ends a program",endsLine(listOf(ProgramIdentity.Loading)))
        assertEquals("ends a program",endsLine(listOf(fish,ProgramIdentity.Terminal)))
        assertEquals("ends cargo, a program",endsLine(listOf(cargo,ProgramIdentity.Loading)))
    }
    @Test fun sessionDetailNamesPanesOnlyWhenATabIsSplit() {
        fun detail(tabs: Int,panes: Int)=SessionEntry(SessionTarget(server,SessionItem("s","S"),SessionDetail("detached",tabs,panes))).detail
        assertEquals("5 tabs · 6 panes",detail(5,6))
        assertEquals("1 tab",detail(1,1))
        assertEquals("2 tabs",detail(2,2))
        assertEquals("",session("s",null).detail)
    }
    @Test fun aTabMenuRenamesSplitsItsShownPaneAndCloses() {
        val rows=RowsState();val manager=RowManager(rows,model())
        assertEquals(listOf("Rename","Split right","Split below","Close"),menu(manager,"tab:w1"))
        run(manager,"tab:w1","Split below");run(manager,"tab:w2","Split right")
        assertEquals(listOf("split:b:BELOW","split:c:RIGHT"),calls)
        run(manager,"tab:w1","Rename")
        assertEquals(TerminalRenameTarget("w1","w1",true,"label-w1"),rows.rename)
    }
    @Test fun aPaneMenuSplitsThatPaneAndFloatsOnlyWhileAnotherStaysDocked() {
        val rows=RowsState()
        assertEquals(listOf("Rename","Split right","Split below","Float","Close"),menu(RowManager(rows,model()),"pane:a"))
        run(RowManager(rows,model()),"pane:a","Split right");run(RowManager(rows,model()),"pane:a","Float")
        assertEquals(listOf("split:a:RIGHT","float:w1:a"),calls)
        // The last docked pane cannot float; a floating pane docks instead.
        val floating=listOf(tab("w1",pane("a"),pane("f",floating=true)))
        assertEquals(listOf("Rename","Split right","Split below","Close"),menu(RowManager(rows,model(floating)),"pane:a"))
        assertEquals(listOf("Rename","Split right","Split below","Dock","Close"),menu(RowManager(rows,model(floating)),"pane:f"))
        run(RowManager(rows,model(floating)),"pane:f","Dock")
        assertEquals("w1" to "f",rows.dock)
        // Without geometry there is nothing to float against.
        assertFalse("Float" in menu(RowManager(rows,model(listOf(tab("w1",pane("a"),pane("b"),layout=false)))),"pane:a"))
    }
    @Test fun closingAsksInTheRowOnlyWhenAProgramWouldEnd() {
        val rows=RowsState();val manager=RowManager(rows,model())
        rows.revealed="tab:w2" to -1
        run(manager,"tab:w2","Close")
        assertEquals(listOf("close-tab:w2"),calls);assertNull(rows.confirming);assertNull(rows.revealed);assertTrue("tab:w2" in rows.closing)
        run(manager,"tab:w1","Close")
        assertEquals(1,calls.size);assertEquals("tab:w1",rows.confirming)
        val close=manager.close("tab:w1")!!
        assertEquals("Close Tab w1?",close.question);assertEquals("ends cargo",close.detail)
        rows.confirmClose("tab:w1",close)
        assertEquals(listOf("close-tab:w2","close-tab:w1"),calls);assertNull(rows.confirming)
        run(manager,"pane:a","Close");run(manager,"pane:d","Close")
        assertEquals(listOf("close-tab:w2","close-tab:w1","close-pane:a"),calls);assertEquals("pane:d",rows.confirming)
    }
    @Test fun aSessionAlwaysAsksAndSaysHowMuchItEnds() {
        val rows=RowsState();val manager=RowManager(rows,model())
        assertEquals(listOf("Rename","Close"),menu(manager,"session:${sessions[0].key}"))
        run(manager,"session:${sessions[0].key}","Close")
        assertTrue(calls.isEmpty());assertEquals("session:${sessions[0].key}",rows.confirming)
        assertEquals("ends 2 tabs",manager.close("session:${sessions[0].key}")!!.detail)
        assertEquals("ends every tab",manager.close("session:${sessions[1].key}")!!.detail)
        run(manager,"session:${sessions[1].key}","Rename")
        assertEquals(sessions[1].target,rows.renameSession)
        // While one session is being closed or renamed, the others wait.
        assertTrue(RowManager(rows,model(busy=sessions[0].key)).actions("session:${sessions[1].key}").none {it.enabled})
    }
    @Test fun rowsReorderAmongTheirOwnKindAndSessionsStayPut() {
        val manager=RowManager(RowsState(),model())
        assertEquals(listOf("tab:w1","tab:w2","tab:w3"),manager.siblings("tab:w2"))
        assertEquals(listOf("pane:a","pane:b"),manager.siblings("pane:b"))
        assertTrue(manager.siblings("session:${sessions[0].key}").isEmpty())
        manager.move("tab:w3","tab:w1");manager.move("tab:w1",null);manager.move("pane:b","pane:a")
        assertEquals(listOf("move-tab:w3:w1","move-tab:w1:null","move-pane:w1:b:a"),calls)
    }
    @Test fun movesAreOfferedToAccessibilityWithinBoundsButKeptOutOfTheMenu() {
        val manager=RowManager(RowsState(),model())
        fun moves(key: String)=manager.actions(key).filterNot {it.menu}.map {it.label}
        assertEquals(listOf("Move down"),moves("tab:w1"));assertEquals(listOf("Move up","Move down"),moves("tab:w2"));assertEquals(listOf("Move up"),moves("tab:w3"))
        run(manager,"tab:w1","Move down");run(manager,"tab:w2","Move down");run(manager,"tab:w3","Move up");run(manager,"pane:a","Move down")
        assertEquals(listOf("move-tab:w1:w3","move-tab:w2:null","move-tab:w3:w2","move-pane:w1:a:null"),calls)
    }
    @Test fun nothingCanBeChangedUntilTheServerIsReady() {
        val manager=RowManager(RowsState(),model(ready=false))
        assertTrue((manager.actions("tab:w1")+manager.actions("pane:a")).none {it.enabled})
    }
    @Test fun stateForRowsThatAreGoneIsDropped() {
        val rows=RowsState()
        rows.menu=RowMenu("tab:w9",0f,0f,0f);rows.revealed="tab:w9" to 1;rows.confirming="pane:z";rows.closing=setOf("tab:w9","tab:w1");rows.expanded=setOf("tab:w1","tab:w9")
        rows.retain(RowManager(rows,model()).keys().toSet())
        assertNull(rows.menu);assertNull(rows.revealed);assertNull(rows.confirming)
        assertEquals(setOf("tab:w1"),rows.closing);assertEquals(setOf("tab:w1"),rows.expanded)
        assertEquals(listOf("tab:w1","pane:a","pane:b","tab:w2","pane:c","tab:w3","pane:d","session:${sessions[0].key}","session:${sessions[1].key}"),RowManager(rows,model()).keys())
    }
}
