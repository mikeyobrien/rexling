package dev.shulhi.rex

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInHorizontally
import androidx.compose.animation.slideOutHorizontally
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.*
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.chrisbanes.haze.HazeState
import kotlinx.coroutines.delay

private enum class SwitcherLevel {TABS,SESSIONS}

/** Past this many, sessions no longer fit on screen at once and the list can be searched. */
private const val SearchableSessions=6
private val GroupShape=Kit.group

/**
 * One card for sessions, tabs and panes. It opens on the session's tabs; a tab with several panes
 * lists them in place, and the header goes up to the sessions. Tapping a row always opens it.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable internal fun Switcher(model: SwitcherModel,onClose: ()->Unit,sessionsFirst: Boolean=false,hazeState: HazeState?=null) {
    var chosen by rememberSaveable {mutableStateOf(if(sessionsFirst) SwitcherLevel.SESSIONS else SwitcherLevel.TABS)}
    // A session with no tabs has nothing to show but the way out.
    val level=if(model.tabs.isEmpty()) SwitcherLevel.SESSIONS else chosen
    val haptics=LocalHapticFeedback.current
    val rows=remember {RowsState().apply {
        arriving=true
        // Open on the panes of the tab you are in, when it has more than one.
        if((model.tabs.firstOrNull {it.id==model.currentTab}?.panes?.size ?: 0)>1) expanded=setOf(tabKey(model.currentTab))
    }}
    val manager=RowManager(rows,model)
    LaunchedEffect(Unit) {haptics.performHapticFeedback(HapticFeedbackType.SegmentTick);delay(ArrivalMillis);rows.arriving=false}
    val refresh by rememberUpdatedState(model.refreshSessions)
    LaunchedEffect(level) {rows.settle();if(level==SwitcherLevel.SESSIONS) refresh()}
    // Built out here on purpose: the frost redraws when its host window draws, and inside the sheet
    // the host would be the sheet's own window, which would then redraw itself every frame.
    val frost=Modifier.sheetSurface(hazeState,Kit.card)
    SheetWindow(onClose,card=true) {
        Box(Modifier.fillMaxWidth().windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom)).padding(horizontal=10.dp).padding(bottom=10.dp)) {
            Box(Modifier.fillMaxWidth().then(frost).dockedCardEdge().padding(12.dp)) {
                AnimatedContent(level,transitionSpec={
                    // Sessions sit above tabs: going up comes in from the left, going down from the right.
                    val down=targetState==SwitcherLevel.TABS
                    (slideInHorizontally(tween(220)) {width->if(down) width/5 else -width/5}+fadeIn(tween(180))).togetherWith(
                        slideOutHorizontally(tween(160)) {width->if(down) -width/5 else width/5}+fadeOut(tween(110))).using(SizeTransform(clip=false))
                },label="Switcher level") {shown->
                    Column(verticalArrangement=Arrangement.spacedBy(10.dp)) {
                        if(shown==SwitcherLevel.TABS) {
                            SwitcherTitle(model.server,model.session) {chosen=SwitcherLevel.SESSIONS}
                            TabsLevel(model,rows,manager)
                        } else {
                            SwitcherTitle(if(model.sessions.map {it.target.server.id}.distinct().size>1) "" else model.server,"Sessions",null)
                            SessionsLevel(model,rows,manager) {chosen=SwitcherLevel.TABS}
                        }
                    }
                }
                // Outside the animation, so it stays put while the levels change.
                Box(Modifier.align(Alignment.TopEnd).padding(top=4.dp).size(44.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha=.6f))
                    .clickable(role=Role.Button,onClick=onClose).semantics {contentDescription="Close switcher"},contentAlignment=Alignment.Center) {
                    ChromeSymbol(ChromeIcon.CLOSE,Modifier.size(18.dp))
                }
            }
            RowMenuHost(rows,manager::actions)
        }
    }
    RowDialogs(rows,model)
}

/** The card's heading. With [onUp] it is the way up to the sessions. */
@Composable private fun SwitcherTitle(overline: String,title: String,onUp: (()->Unit)?) {
    val dim=MaterialTheme.colorScheme.onSurfaceVariant
    Row(Modifier.padding(end=52.dp).heightIn(min=52.dp).clip(RoundedCornerShape(18.dp))
        .then(if(onUp!=null) Modifier.clickable(role=Role.Button,onClick=onUp).semantics {contentDescription="Sessions"} else Modifier)
        .padding(start=if(onUp!=null) 2.dp else 10.dp,end=10.dp),verticalAlignment=Alignment.CenterVertically) {
        if(onUp!=null) ChromeSymbol(ChromeIcon.CHEVRON,Modifier.padding(end=6.dp).size(20.dp).graphicsLayer {rotationZ=90f},dim)
        Column {
            if(overline.isNotBlank()) Text(overline,fontSize=12.sp,fontWeight=FontWeight.Medium,color=dim,maxLines=1,overflow=TextOverflow.Ellipsis)
            Text(title,fontSize=20.sp,fontWeight=FontWeight.Bold,maxLines=1,overflow=TextOverflow.Ellipsis)
        }
    }
}

@Composable private fun TabsLevel(model: SwitcherModel,rows: RowsState,manager: RowManager) {
    // Start with the tab you are in on screen, however long the list.
    val list=rememberLazyListState((model.tabs.indexOfFirst {it.id==model.currentTab}-2).coerceAtLeast(0))
    ManagedList(rows,list,model.ready,manager::actions,manager::siblings,manager::move,manager.keys(),
        Modifier.fillMaxWidth().heightIn(max=sheetBodyMax(432.dp,chrome=300.dp)).clip(GroupShape)
            .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha=.55f)).semantics {contentDescription="Tabs"}) {
        tabRows(rows,list,model,manager)
    }
    if(model.error.isNotBlank()) Text(model.error,Modifier.padding(horizontal=10.dp),fontSize=12.sp,color=MaterialTheme.colorScheme.error,maxLines=2,overflow=TextOverflow.Ellipsis)
    Row(Modifier.fillMaxWidth(),horizontalArrangement=Arrangement.spacedBy(8.dp)) {
        PillButton(ChromeIcon.ADD,"New tab",model.canCreate,Modifier.weight(1f),onClick=model.newTab)
        PaneSplit.entries.forEach {split->SplitButton(split,model.canSplit,52.dp,CircleShape) {model.split(model.currentPane,split)}}
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable private fun SessionsLevel(model: SwitcherModel,rows: RowsState,manager: RowManager,onTabs: ()->Unit) {
    val scheme=MaterialTheme.colorScheme
    var search by rememberSaveable {mutableStateOf("")}
    val list=rememberLazyListState()
    val searchable=model.sessions.size>SearchableSessions
    val entries=model.sessions.associateBy {it.key}
    val groups=SessionCatalog.groups(model.sessions.map {it.target},model.currentSession,if(searchable) search else "")
    val servers=model.sessions.map {it.target.server.id}.distinct().size
    if(searchable) BasicTextField(search,{search=it},Modifier.fillMaxWidth().height(44.dp).clip(CircleShape).background(scheme.surfaceVariant.copy(alpha=.55f)).padding(horizontal=16.dp),
        textStyle=TextStyle(fontSize=15.sp,color=scheme.onSurface),singleLine=true,cursorBrush=SolidColor(scheme.primary),
        decorationBox={field->Row(Modifier.fillMaxHeight(),verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(10.dp)) {
            ChromeSymbol(ChromeIcon.SEARCH,Modifier.size(18.dp),scheme.onSurfaceVariant)
            Box(Modifier.weight(1f)) {if(search.isEmpty()) Text("Find",fontSize=15.sp,color=scheme.onSurfaceVariant.copy(alpha=.7f));field()}
        }})
    if(groups.isEmpty()) Text(if(model.sessions.isEmpty()) "No sessions yet" else "No matching sessions",Modifier.padding(horizontal=10.dp,vertical=14.dp),
        fontSize=14.sp,color=scheme.onSurfaceVariant)
    // Searching filters inside a list of fixed height, so the field never moves under the finger.
    val height=sheetBodyMax(432.dp,chrome=if(searchable) 360.dp else 300.dp)
    // Pulling the list down asks every server again; nothing reports back, so the spinner just says it was heard.
    var refreshing by remember {mutableStateOf(false)}
    LaunchedEffect(refreshing) {if(refreshing) {model.refreshSessions();delay(700);refreshing=false}}
    if(groups.isNotEmpty() || searchable) PullToRefreshBox(refreshing,{refreshing=true}) {ManagedList(rows,list,true,manager::actions,manager::siblings,manager::move,manager.keys(),
        Modifier.fillMaxWidth().then(if(searchable) Modifier.height(minOf(height,56.dp*model.sessions.size)) else Modifier.heightIn(max=height)).clip(GroupShape)
            .background(scheme.surfaceVariant.copy(alpha=.55f)).semantics {contentDescription="Sessions list"}) {
        groups.forEachIndexed {index,group->
            // With one server, naming it above its sessions is noise.
            if(servers>1) item(key="server:${group.server.id}",contentType="server") {
                Row(Modifier.fillMaxWidth().padding(start=18.dp,end=16.dp,top=if(index==0) 12.dp else 14.dp,bottom=2.dp),verticalAlignment=Alignment.CenterVertically) {
                    ServerGlyph(group.server.icon,Modifier.size(16.dp))
                    Text(group.server.label,Modifier.padding(start=8.dp),fontSize=12.sp,fontWeight=FontWeight.SemiBold,color=scheme.onSurfaceVariant,maxLines=1,overflow=TextOverflow.Ellipsis)
                }
            }
            sessionRows(rows,group.sessions.mapNotNull {entries[it.key]},model,manager,
                // The session you are in is already open: show its tabs.
                onOpen={entry->if(entry.key==model.currentSession && model.tabs.isNotEmpty()) onTabs() else model.openSession(entry.key)})
        }
    }    }
    if(model.sessionError.isNotBlank()) Text(model.sessionError,Modifier.padding(horizontal=10.dp),fontSize=12.sp,color=scheme.error,maxLines=2,overflow=TextOverflow.Ellipsis)
    PillButton(ChromeIcon.ADD,"New session",model.ready,Modifier.fillMaxWidth(),onClick=model.newSession)
}

@Composable private fun PillButton(icon: ChromeIcon,label: String,enabled: Boolean,modifier: Modifier=Modifier,onClick: ()->Unit) =
    KitButton(label,onClick,modifier.height(52.dp),enabled=enabled,icon=icon)
