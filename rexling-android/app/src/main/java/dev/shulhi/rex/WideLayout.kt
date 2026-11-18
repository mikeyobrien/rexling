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

/**
 * The list's width: on a near-square screen (an unfolded book-style foldable) exactly half, so the
 * hinge falls between list and terminal instead of through terminal text; otherwise a fixed rail.
 */
internal fun wideListWidth(width: Dp,height: Dp): Dp {
    val aspect=width/height
    return if(aspect in .75f..1.34f) width/2 else 320.dp
}

/** List-detail on wide screens: the session's tabs, the other sessions, and quick actions. */
@Composable internal fun TabListPanel(width: Dp,session: String,tabs: List<TabEntry>,current: String,otherSessions: List<SessionItem>,
    enabled: Boolean,canCreate: Boolean,activityUnread: Boolean,
    onTab: (String)->Unit,onSession: (String)->Unit,onSessions: ()->Unit,onActivity: ()->Unit,onSettings: ()->Unit,onNewTab: ()->Unit) {
    Box(Modifier.width(width).fillMaxHeight().padding(start=8.dp,end=4.dp,bottom=8.dp)) {
        Column(Modifier.fillMaxSize().clip(RoundedCornerShape(26.dp)).background(MaterialTheme.colorScheme.surface)) {
            Text("Rexling",Modifier.padding(start=20.dp,top=16.dp,bottom=8.dp),fontSize=22.sp,fontWeight=FontWeight.Bold)
            LazyColumn(Modifier.fillMaxWidth().weight(1f).semantics {contentDescription="Tab list"},
                contentPadding=PaddingValues(start=8.dp,end=8.dp,bottom=88.dp)) {
                item {
                    Text(session,Modifier.padding(start=12.dp,top=4.dp,bottom=4.dp),fontSize=12.sp,fontWeight=FontWeight.SemiBold,
                        color=MaterialTheme.colorScheme.onSurfaceVariant,fontFamily=MonoUi,maxLines=1,overflow=TextOverflow.Ellipsis)
                }
                items(tabs,key={it.id}) {tab->
                    val active=tab.id==current
                    Row(Modifier.fillMaxWidth().padding(vertical=2.dp).clip(RoundedCornerShape(20.dp))
                        .background(if(active) MaterialTheme.colorScheme.surfaceVariant.copy(alpha=.7f) else MaterialTheme.colorScheme.surface)
                        .clickable(enabled=enabled,role=Role.Button) {onTab(tab.id)}
                        .semantics {selected=active;stateDescription=if(active) "Current tab" else ""}
                        .padding(horizontal=12.dp,vertical=10.dp),
                        verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(12.dp)) {
                        Box(Modifier.width(48.dp)) {ProgramStack(tab.programs,tab.selectedPane,framed=true,fixedSlot=true)}
                        Column(Modifier.weight(1f)) {
                            Text(tab.title,fontSize=17.sp,fontWeight=FontWeight.SemiBold,maxLines=1,overflow=TextOverflow.Ellipsis)
                            if(tab.detail.isNotBlank()) Text(tab.detail,fontSize=12.sp,fontFamily=MonoUi,color=MaterialTheme.colorScheme.onSurfaceVariant,
                                maxLines=1,overflow=TextOverflow.Ellipsis)
                        }
                        TabStatus(tab)
                    }
                }
                if(otherSessions.isNotEmpty()) item {
                    HorizontalDivider(Modifier.padding(horizontal=12.dp,vertical=10.dp),color=MaterialTheme.colorScheme.outline.copy(alpha=.25f))
                }
                items(otherSessions,key={"session:"+it.id}) {item->
                    Row(Modifier.fillMaxWidth().padding(vertical=2.dp).clip(RoundedCornerShape(20.dp))
                        .clickable(enabled=enabled,role=Role.Button) {onSession(item.id)}
                        .semantics {contentDescription="Session ${item.label}"}.padding(horizontal=12.dp,vertical=12.dp),
                        verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(12.dp)) {
                        Box(Modifier.width(48.dp),contentAlignment=Alignment.Center) {
                            ChromeSymbol(ChromeIcon.SESSIONS,Modifier.size(20.dp),MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                        Text(item.label,Modifier.weight(1f),fontSize=15.sp,fontFamily=MonoUi,maxLines=1,overflow=TextOverflow.Ellipsis)
                    }
                }
            }
        }
        // Floating actions, as on T3: navigation pill and a prominent New tab.
        Row(Modifier.align(Alignment.BottomStart).fillMaxWidth().padding(12.dp),verticalAlignment=Alignment.CenterVertically,
            horizontalArrangement=Arrangement.spacedBy(10.dp)) {
            Row(Modifier.clip(RoundedCornerShape(28.dp)).background(MaterialTheme.colorScheme.surfaceVariant).padding(4.dp),
                horizontalArrangement=Arrangement.spacedBy(2.dp)) {
                NavButton(ChromeIcon.SESSIONS,"Sessions",enabled,onClick=onSessions)
                NavButton(ChromeIcon.EYE,"Activity",enabled,dot=activityUnread,onClick=onActivity)
                NavButton(ChromeIcon.SETTINGS,"Preferences",true,onClick=onSettings)
            }
            Spacer(Modifier.weight(1f))
            Box(Modifier.size(56.dp).clip(RoundedCornerShape(18.dp)).background(MaterialTheme.colorScheme.primary)
                .clickable(enabled=canCreate,role=Role.Button,onClick=onNewTab).semantics {contentDescription="New tab"},contentAlignment=Alignment.Center) {
                ChromeSymbol(ChromeIcon.ADD,Modifier.size(24.dp),MaterialTheme.colorScheme.onPrimary)
            }
        }
    }
}

@Composable private fun NavButton(icon: ChromeIcon,label: String,enabled: Boolean,dot: Boolean=false,onClick: ()->Unit) {
    Box(Modifier.size(48.dp).clip(CircleShape).clickable(enabled=enabled,role=Role.Button,onClick=onClick)
        .semantics {contentDescription=label},contentAlignment=Alignment.Center) {
        ChromeSymbol(icon,Modifier.size(20.dp),MaterialTheme.colorScheme.onSurface.copy(alpha=if(enabled) 1f else .38f))
        if(dot) Box(Modifier.align(Alignment.Center).offset(x=8.dp,y=(-6).dp).size(8.dp).background(MaterialTheme.colorScheme.primary,CircleShape))
    }
}
