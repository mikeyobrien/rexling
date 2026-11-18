package dev.shulhi.rex

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
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

/** "~" alone or a blank folder says nothing; show the detail line only when it does. */
private fun informative(detail: String)=detail.isNotBlank() && detail.trim()!="~"

/** Status beside a tab title: a tinted "Needs you" chip, or a spinner while it works. */
@Composable private fun TabStatusChip(tab: TabEntry) {
    when {
        tab.attention -> Text("Needs you",Modifier.clip(RoundedCornerShape(8.dp)).background(MaterialTheme.colorScheme.primary.copy(alpha=.18f))
            .padding(horizontal=8.dp,vertical=3.dp),fontSize=11.sp,fontWeight=FontWeight.Medium,color=MaterialTheme.colorScheme.primary)
        tab.working -> Row(verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(5.dp)) {
            CircularProgressIndicator(Modifier.size(11.dp),strokeWidth=1.5.dp,color=MaterialTheme.colorScheme.tertiary)
            Text("Working",fontSize=11.sp,fontWeight=FontWeight.Medium,color=MaterialTheme.colorScheme.tertiary)
        }
    }
}

/** List-detail on wide screens: the session's tabs, the other sessions, and quick actions. */
@Composable internal fun TabListPanel(width: Dp,session: String,server: String,tabs: List<TabEntry>,current: String,otherSessions: List<SessionItem>,
    enabled: Boolean,canCreate: Boolean,
    onTab: (String)->Unit,onSession: (String)->Unit,onSessions: ()->Unit,onSettings: ()->Unit,onNewTab: ()->Unit,onCollapse: ()->Unit) {
    val dim=MaterialTheme.colorScheme.onSurfaceVariant
    Box(Modifier.width(width).fillMaxHeight().padding(start=8.dp,end=4.dp,bottom=8.dp)) {
        Column(Modifier.fillMaxSize().clip(WideCardShape).background(MaterialTheme.colorScheme.surface)) {
            // Header: the session is the title; the app name adds nothing here.
            Row(Modifier.fillMaxWidth().heightIn(min=56.dp).padding(start=6.dp,end=4.dp),verticalAlignment=Alignment.CenterVertically) {
                Column(Modifier.weight(1f).clip(RoundedCornerShape(14.dp)).clickable(enabled=enabled,role=Role.Button,onClick=onSessions)
                    .semantics {contentDescription="Switch session"}.padding(horizontal=10.dp,vertical=6.dp)) {
                    Row(verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(4.dp)) {
                        Text(session,Modifier.weight(1f,fill=false),fontSize=20.sp,fontWeight=FontWeight.SemiBold,maxLines=1,overflow=TextOverflow.Ellipsis)
                        ChromeSymbol(ChromeIcon.CHEVRON,Modifier.size(16.dp),dim)
                    }
                    if(server.isNotBlank()) Text(server,fontSize=12.sp,color=dim,maxLines=1,overflow=TextOverflow.Ellipsis)
                }
                ChromeButton(ChromeIcon.CHEVRON,"Hide tab list",modifier=Modifier.graphicsLayer {rotationZ=90f},onClick=onCollapse)
            }
            LazyColumn(Modifier.fillMaxWidth().weight(1f).semantics {contentDescription="Tab list"},
                contentPadding=PaddingValues(start=8.dp,end=8.dp,bottom=80.dp)) {
                items(tabs,key={it.id}) {tab->
                    val active=tab.id==current
                    Row(Modifier.fillMaxWidth().heightIn(min=56.dp).padding(vertical=1.dp).clip(RoundedCornerShape(14.dp))
                        .background(if(active) MaterialTheme.colorScheme.primary.copy(alpha=.10f) else MaterialTheme.colorScheme.surface)
                        .clickable(enabled=enabled,role=Role.Button) {onTab(tab.id)}
                        .semantics {selected=active;stateDescription=if(active) "Current tab" else ""},
                        verticalAlignment=Alignment.CenterVertically) {
                        Box(Modifier.width(3.dp).height(32.dp).background(if(active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.surface.copy(alpha=0f),
                            RoundedCornerShape(2.dp)))
                        Row(Modifier.weight(1f).padding(start=7.dp,end=10.dp,top=6.dp,bottom=6.dp),verticalAlignment=Alignment.CenterVertically,
                            horizontalArrangement=Arrangement.spacedBy(10.dp)) {
                            Box(Modifier.width(44.dp)) {ProgramStack(tab.programs,tab.selectedPane,framed=true,fixedSlot=true)}
                            Column(Modifier.weight(1f)) {
                                Row(verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(8.dp)) {
                                    Text(tab.title,Modifier.weight(1f),fontSize=15.sp,fontWeight=FontWeight.SemiBold,maxLines=1,overflow=TextOverflow.Ellipsis)
                                    TabStatusChip(tab)
                                }
                                if(informative(tab.detail)) Text(tab.detail,fontSize=12.sp,fontFamily=MonoUi,color=dim,maxLines=1,overflow=TextOverflow.Ellipsis)
                            }
                        }
                    }
                }
                if(otherSessions.isNotEmpty()) item {
                    Text("SESSIONS",Modifier.padding(start=14.dp,top=18.dp,bottom=4.dp),fontSize=11.sp,fontWeight=FontWeight.SemiBold,
                        letterSpacing=1.2.sp,color=dim)
                }
                items(otherSessions,key={"session:"+it.id}) {item->
                    Row(Modifier.fillMaxWidth().heightIn(min=44.dp).clip(RoundedCornerShape(14.dp))
                        .clickable(enabled=enabled,role=Role.Button) {onSession(item.id)}
                        .semantics {contentDescription="Session ${item.label}"}.padding(horizontal=14.dp),
                        verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(12.dp)) {
                        Box(Modifier.size(6.dp).background(dim,CircleShape))
                        Text(item.label,Modifier.weight(1f),fontSize=14.sp,maxLines=1,overflow=TextOverflow.Ellipsis)
                    }
                }
            }
        }
        // Light navigation pill (Activity lives in the terminal header) and a New tab button.
        Row(Modifier.align(Alignment.BottomStart).fillMaxWidth().padding(12.dp),verticalAlignment=Alignment.CenterVertically) {
            Row(Modifier.clip(RoundedCornerShape(24.dp)).background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha=.55f)).padding(3.dp),
                horizontalArrangement=Arrangement.spacedBy(2.dp)) {
                NavButton(ChromeIcon.SESSIONS,"Sessions",enabled,onClick=onSessions)
                NavButton(ChromeIcon.SETTINGS,"Preferences",true,onClick=onSettings)
            }
            Spacer(Modifier.weight(1f))
            Box(Modifier.size(48.dp).clip(RoundedCornerShape(16.dp)).background(MaterialTheme.colorScheme.primary)
                .clickable(enabled=canCreate,role=Role.Button,onClick=onNewTab).semantics {contentDescription="New tab"},contentAlignment=Alignment.Center) {
                ChromeSymbol(ChromeIcon.ADD,Modifier.size(22.dp),MaterialTheme.colorScheme.onPrimary)
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
