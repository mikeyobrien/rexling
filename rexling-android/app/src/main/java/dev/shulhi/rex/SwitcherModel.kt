package dev.shulhi.rex

import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/** One pane inside a tab, as the switcher shows it. */
internal data class PaneEntry(val id: String,val title: String,val detail: String,val label: String,val program: ProgramIdentity,
    val floating: Boolean=false,val working: Boolean=false)

/** One tab as the switcher shows it. [label] is the name the user gave it, blank when automatic. */
internal data class TabEntry(val id: String,val title: String,val detail: String,val programs: List<ProgramIdentity>,
    val selectedPane: Int,val attention: Boolean,val working: Boolean,
    val label: String="",val panes: List<PaneEntry> = emptyList(),val layout: List<PanePlacement> = emptyList())

/** One session as the switcher shows it. */
internal data class SessionEntry(val target: SessionTarget,val attention: Boolean=false,val working: Boolean=false) {
    val key: String get()=target.key
    /** "5 tabs · 6 panes"; panes only when a tab is split, blank until the server reports the counts. */
    val detail: String get()=target.detail?.let {d->listOfNotNull(counted(d.tabs,"tab"),if(d.panes>d.tabs) counted(d.panes,"pane") else null).joinToString(" · ")}.orEmpty()
}
private fun counted(count: Int,noun: String)="$count $noun${if(count==1) "" else "s"}"

/** "~" alone or a blank folder says nothing; show the detail line only when it does. */
internal fun informative(detail: String)=detail.isNotBlank() && detail.trim()!="~"

/**
 * What closing would end, as the confirmation's second line: the programs other than idle shells.
 * Blank means only shells at their prompt, so the row closes without asking. A pane that has not
 * been identified could be running anything, so it asks too.
 */
internal fun endsLine(programs: List<ProgramIdentity>): String {
    val known=programs.filterNot {it.idle || it.unknown}.map {it.title}.distinct()
    val running=if(programs.any {it.unknown}) known+"a program" else known
    return when {running.isEmpty()->"";running.size>2->"ends ${running.size} programs";else->"ends ${running.joinToString(", ")}"}
}

/** One thing a row can do. [menu] false keeps it out of the hold menu: an accessibility action only. */
internal class RowAction(val label: String,val icon: ChromeIcon?=null,val split: PaneSplit?=null,val destructive: Boolean=false,
    val enabled: Boolean=true,val menu: Boolean=true,val run: ()->Unit)

/** How a row closes. A non-blank [detail] ("ends cargo") is asked in the row before [run]. */
internal class RowClose(val question: String,val detail: String,val run: ()->Unit)

/** Where a held row sits, in root pixels, so its menu can open beside it. */
internal data class RowMenu(val key: String,val top: Float,val bottom: Float,val right: Float)

/**
 * A managed list's transient state: at most one hold menu, one revealed Close, one confirmation
 * and one drag. Row keys are "tab:id", "pane:id" and "session:key".
 */
@Stable internal class RowsState {
    var menu by mutableStateOf<RowMenu?>(null)
    /** The row whose Close is revealed, and on which side: 1 leading, -1 trailing. */
    var revealed by mutableStateOf<Pair<String,Int>?>(null)
    var confirming by mutableStateOf<String?>(null)
    /** Rows whose close was sent: dimmed until the server drops them. */
    var closing by mutableStateOf<Set<String>>(emptySet())
    /** Tabs showing their panes. */
    var expanded by mutableStateOf<Set<String>>(emptySet())
    var rename by mutableStateOf<TerminalRenameTarget?>(null)
    var renameSession by mutableStateOf<SessionTarget?>(null)
    /** Tab and pane being docked. */
    var dock by mutableStateOf<Pair<String,String>?>(null)
    val drag=TabDrag()

    /** Close now, or ask in the row first when that would end a program. */
    fun requestClose(key: String,close: RowClose) {
        menu=null;revealed=null
        if(close.detail.isBlank()) confirmClose(key,close) else confirming=key
    }
    fun confirmClose(key: String,close: RowClose) {confirming=null;closing=closing+key;close.run()}
    /** Put every row back to rest. */
    fun settle() {menu=null;revealed=null;confirming=null}
    /** Drop state that points at rows the list no longer has. */
    fun retain(keys: Set<String>) {
        if(menu?.key?.let {it !in keys}==true) menu=null
        if(revealed?.first?.let {it !in keys}==true) revealed=null
        if(confirming?.let {it !in keys}==true) confirming=null
        if(closing.any {it !in keys}) closing=closing.filterTo(HashSet()) {it in keys}
        if(expanded.any {it !in keys}) expanded=expanded.filterTo(HashSet()) {it in keys}
    }
}

/** Everything the switcher shows and can do. The card and the wide list share it. */
internal class SwitcherModel(
    val server: String="",val session: String="",val tabs: List<TabEntry> = emptyList(),val currentTab: String="",val currentPane: String="",
    val sessions: List<SessionEntry> = emptyList(),val currentSession: String="",
    /** Rows can be opened. */
    val enabled: Boolean=true,
    /** Tabs and panes can be changed: renamed, split, moved or closed. */
    val ready: Boolean=true,
    val canCreate: Boolean=true,val canSplit: Boolean=true,
    /** The session being closed or renamed, if any. */
    val busySession: String="",val error: String="",val sessionError: String="",
    val openTab: (String)->Unit={},val openPane: (String)->Unit={},val newTab: ()->Unit={},val split: (String,PaneSplit)->Unit={_,_->},
    val closeTab: (TabEntry)->Unit={},val closePane: (PaneEntry)->Unit={},
    val renameTab: (String,String)->Boolean={_,_->false},val renamePane: (String,String,String)->Boolean={_,_,_->false},
    val moveTab: (String,String?)->Boolean={_,_->false},val movePane: (String,String,String?)->Boolean={_,_,_->false},
    val floatPane: (String,String)->Boolean={_,_->false},val dockPane: (String,String,String,PaneSplit)->Boolean={_,_,_,_->false},
    val openSession: (String)->Unit={},val newSession: ()->Unit={},val refreshSessions: ()->Unit={},
    val closeSession: (SessionTarget)->Unit={},val renameSession: (SessionTarget,String)->Boolean={_,_->false})

/** What each row of a [SwitcherModel] can do: its menu, how it closes and what it reorders among. */
internal class RowManager(private val rows: RowsState,private val model: SwitcherModel) {
    private fun tab(key: String)=model.tabs.firstOrNull {tabKey(it.id)==key}
    private fun pane(key: String): Pair<TabEntry,PaneEntry>? {
        for(tab in model.tabs) tab.panes.firstOrNull {paneKey(it.id)==key}?.let {return tab to it}
        return null
    }
    private fun session(key: String)=model.sessions.firstOrNull {sessionRowKey(it.key)==key}
    private fun splits(anchor: String?)=PaneSplit.entries.map {split->
        RowAction(split.label,split=split,enabled=model.ready && model.canCreate && anchor!=null) {anchor?.let {model.split(it,split)}}
    }
    private fun closeAction(key: String,enabled: Boolean)=RowAction("Close",ChromeIcon.CLOSE,destructive=true,enabled=enabled) {close(key)?.let {rows.requestClose(key,it)}}
    /** Reordering for those who can't drag: "move this before that", null being the end. */
    private fun moves(ids: List<String>,id: String,move: (String?)->Unit): List<RowAction> {
        val index=ids.indexOf(id)
        return listOfNotNull(
            if(index>0) RowAction("Move up",enabled=model.ready,menu=false) {move(ids[index-1])} else null,
            if(index in 0 until ids.lastIndex) RowAction("Move down",enabled=model.ready,menu=false) {move(ids.getOrNull(index+2))} else null)
    }

    fun actions(key: String): List<RowAction> {
        tab(key)?.let {tab->
            return listOf(RowAction("Rename",ChromeIcon.EDIT,enabled=model.ready) {rows.rename=TerminalRenameTarget(tab.id,tab.id,true,tab.label)})+
                splits(tab.panes.getOrNull(tab.selectedPane)?.id)+closeAction(key,model.ready)+
                moves(model.tabs.map {it.id},tab.id) {before->model.moveTab(tab.id,before)}
        }
        pane(key)?.let {(tab,pane)->
            val docked=tab.panes.filterNot {it.floating}
            return listOf(RowAction("Rename",ChromeIcon.EDIT,enabled=model.ready) {rows.rename=TerminalRenameTarget(tab.id,pane.id,false,pane.label)})+
                splits(pane.id)+listOfNotNull(
                    // A tab keeps at least one docked pane, so the last one cannot float.
                    if(pane.floating) RowAction("Dock",ChromeIcon.SPLIT,enabled=model.ready && docked.isNotEmpty()) {rows.dock=tab.id to pane.id}
                    else if(docked.size>1 && tab.layout.any {it.id==pane.id}) RowAction("Float",ChromeIcon.FLOAT,enabled=model.ready) {model.floatPane(tab.id,pane.id)} else null)+
                closeAction(key,model.ready)+moves(tab.panes.map {it.id},pane.id) {before->model.movePane(tab.id,pane.id,before)}
        }
        session(key)?.let {entry->
            val free=model.busySession.isBlank()
            return listOf(RowAction("Rename",ChromeIcon.EDIT,enabled=free) {rows.renameSession=entry.target},closeAction(key,free))
        }
        return emptyList()
    }

    fun close(key: String): RowClose? {
        tab(key)?.let {tab->return RowClose("Close ${tab.title}?",endsLine(tab.programs)) {model.closeTab(tab)}}
        pane(key)?.let {(_,pane)->return RowClose("Close ${pane.title}?",endsLine(listOf(pane.program))) {model.closePane(pane)}}
        // A session always asks: it takes every tab with it.
        session(key)?.let {entry->return RowClose("Close ${entry.target.session.label}?",
            entry.target.detail?.let {"ends ${counted(it.tabs,"tab")}"} ?: "ends every tab") {model.closeSession(entry.target)}}
        return null
    }

    /** The rows a held row can be dragged among; sessions keep the server's order. */
    fun siblings(key: String): List<String> {
        if(tab(key)!=null) return model.tabs.map {tabKey(it.id)}
        return pane(key)?.first?.panes?.map {paneKey(it.id)}.orEmpty()
    }

    fun move(key: String,before: String?) {
        tab(key)?.let {model.moveTab(it.id,before?.removePrefix("tab:"));return}
        pane(key)?.let {(tab,pane)->model.movePane(tab.id,pane.id,before?.removePrefix("pane:"))}
    }

    /** Every row the model can show, in order: a change means held rows and their state may be stale. */
    fun keys(): List<String> = buildList {
        model.tabs.forEach {tab->add(tabKey(tab.id));tab.panes.forEach {add(paneKey(it.id))}}
        model.sessions.forEach {add(sessionRowKey(it.key))}
    }
}
internal fun tabKey(id: String)="tab:$id"
internal fun paneKey(id: String)="pane:$id"
internal fun sessionRowKey(key: String)="session:$key"
