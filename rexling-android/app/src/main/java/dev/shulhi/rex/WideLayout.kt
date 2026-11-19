package dev.shulhi.rex

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

/** Wide screens show the tab list beside the terminal instead of behind the title. */
internal val WideLayoutMinWidth=600.dp

/** Wide layout cards: the list card and the terminal card share this shape and spacing. */
internal val WideCardShape=RoundedCornerShape(26.dp)

/**
 * The list's width: on a near-square screen (an unfolded book-style foldable) exactly half, so the
 * hinge falls between list and terminal instead of through terminal text; otherwise a fixed rail.
 */
internal fun wideListWidth(width: Dp,height: Dp): Dp {
    val aspect=width/height
    return if(aspect in .75f..1.34f) width/2 else 320.dp
}

/** The list collapsed: one tile per tab, so every tab stays a tap away. */
internal val WideRailWidth=72.dp

/** List-detail on wide screens: the session's tabs, the other sessions, and quick actions. Rows are managed in place, as in the switcher. */
@Composable internal fun TabListPanel(width: Dp,model: SwitcherModel,onSessions: ()->Unit,onSettings: ()->Unit,onCollapse: ()->Unit) {
    val dim=MaterialTheme.colorScheme.onSurfaceVariant
    val rows=remember {RowsState()}
    val manager=RowManager(rows,model)
    val list=rememberLazyListState()
    val others=model.sessions.filter {it.key!=model.currentSession}
    Box(Modifier.width(width).fillMaxHeight().padding(start=8.dp,end=4.dp,bottom=8.dp)) {
        Column(Modifier.fillMaxSize().clip(WideCardShape).background(MaterialTheme.colorScheme.surface)) {
            // Header: the session is the title; the app name adds nothing here.
            Row(Modifier.fillMaxWidth().heightIn(min=56.dp).padding(start=6.dp,end=4.dp),verticalAlignment=Alignment.CenterVertically) {
                Column(Modifier.weight(1f).clip(RoundedCornerShape(14.dp)).clickable(enabled=model.enabled,role=Role.Button,onClick=onSessions)
                    .semantics {contentDescription="Switch session"}.padding(horizontal=10.dp,vertical=6.dp)) {
                    Row(verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(4.dp)) {
                        Text(model.session,Modifier.weight(1f,fill=false),fontSize=20.sp,fontWeight=FontWeight.SemiBold,maxLines=1,overflow=TextOverflow.Ellipsis)
                        ChromeSymbol(ChromeIcon.CHEVRON,Modifier.size(16.dp),dim)
                    }
                    if(model.server.isNotBlank()) Text(model.server,fontSize=12.sp,color=dim,maxLines=1,overflow=TextOverflow.Ellipsis)
                }
                Box(Modifier.size(36.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha=.6f))
                    .clickable(enabled=model.canCreate,role=Role.Button,onClick=model.newTab).semantics {contentDescription="New tab"},contentAlignment=Alignment.Center) {
                    ChromeSymbol(ChromeIcon.ADD,Modifier.size(18.dp),MaterialTheme.colorScheme.onSurface.copy(alpha=if(model.canCreate) 1f else .38f))
                }
                ChromeButton(ChromeIcon.CHEVRON,"Hide tab list",modifier=Modifier.graphicsLayer {rotationZ=90f},onClick=onCollapse)
            }
            // The list stays on screen, so its working dots do not pulse: a pulse here would redraw
            // every frame while any tab works (battery, and the app never goes idle).
            ManagedList(rows,list,model.ready,manager::actions,manager::siblings,manager::move,manager.keys(),
                Modifier.fillMaxWidth().weight(1f).semantics {contentDescription="Tab list"},contentPadding=PaddingValues(start=4.dp,end=4.dp,bottom=8.dp)) {
                tabRows(rows,list,model,manager,pulse=false,dividers=false)
                if(others.isNotEmpty()) item(key="sessions",contentType="label") {
                    Text("SESSIONS",Modifier.padding(start=18.dp,top=18.dp,bottom=4.dp),fontSize=11.sp,fontWeight=FontWeight.SemiBold,
                        letterSpacing=1.2.sp,color=dim)
                }
                sessionRows(rows,others,model,manager,onOpen={model.openSession(it.key)},pulse=false,dividers=false)
            }
            // The footer is part of the card, so it never sits on a row.
            HorizontalDivider(Modifier.padding(horizontal=16.dp),color=MaterialTheme.colorScheme.outline.copy(alpha=.2f))
            Row(Modifier.fillMaxWidth().padding(horizontal=10.dp,vertical=4.dp),verticalAlignment=Alignment.CenterVertically) {
                NavButton(ChromeIcon.SESSIONS,"Sessions",model.enabled,onClick=onSessions)
                NavButton(ChromeIcon.SETTINGS,"Preferences",true,onClick=onSettings)
                Spacer(Modifier.weight(1f))
                PaneSplit.entries.forEach {split->SplitButton(split,model.canSplit,42.dp,CircleShape) {model.split(model.currentPane,split)}}
            }
        }
        RowMenuHost(rows,manager::actions)
    }
    RowDialogs(rows,model)
}

/** The collapsed list: each tab as its program tile with its status dot. */
@Composable internal fun TabRail(model: SwitcherModel,onExpand: ()->Unit) {
    val scheme=MaterialTheme.colorScheme
    Column(Modifier.width(WideRailWidth).fillMaxHeight().padding(start=8.dp,end=4.dp,bottom=8.dp).clip(WideCardShape).background(scheme.surface)
        .semantics {contentDescription="Tab rail"},horizontalAlignment=Alignment.CenterHorizontally) {
        Box(Modifier.height(56.dp),contentAlignment=Alignment.Center) {
            ChromeButton(ChromeIcon.CHEVRON,"Show tab list",modifier=Modifier.graphicsLayer {rotationZ=-90f},onClick=onExpand)
        }
        LazyColumn(Modifier.weight(1f),rememberLazyListState((model.tabs.indexOfFirst {it.id==model.currentTab}-3).coerceAtLeast(0)),
            horizontalAlignment=Alignment.CenterHorizontally,verticalArrangement=Arrangement.spacedBy(6.dp),contentPadding=PaddingValues(bottom=8.dp)) {
            items(model.tabs,key={it.id}) {tab->
                val current=tab.id==model.currentTab
                Box(Modifier.size(52.dp).clip(RoundedCornerShape(16.dp)).background(if(current) scheme.primary.copy(alpha=.16f) else Color.Transparent)
                    .clickable(enabled=model.enabled,role=Role.Tab) {model.openTab(tab.id)}
                    .semantics {contentDescription=tab.title;selected=current},contentAlignment=Alignment.Center) {
                    ProgramStack(tab.programs,tab.selectedPane,framed=true,fixedSlot=true)
                    // Not pulsing, for the same reason as the list.
                    if(tab.attention || tab.working) Box(Modifier.align(Alignment.TopEnd).padding(6.dp).size(9.dp).clip(CircleShape)
                        .background(scheme.surface).padding(1.5.dp).clip(CircleShape).background(if(tab.attention) attentionColor() else scheme.primary))
                }
            }
        }
    }
}

@Composable private fun NavButton(icon: ChromeIcon,label: String,enabled: Boolean,onClick: ()->Unit) {
    Box(Modifier.size(42.dp).clip(CircleShape).clickable(enabled=enabled,role=Role.Button,onClick=onClick)
        .semantics {contentDescription=label},contentAlignment=Alignment.Center) {
        ChromeSymbol(icon,Modifier.size(19.dp),MaterialTheme.colorScheme.onSurface.copy(alpha=if(enabled) .9f else .38f))
    }
}
