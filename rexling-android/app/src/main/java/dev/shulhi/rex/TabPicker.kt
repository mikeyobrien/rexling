package dev.shulhi.rex

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.stateDescription
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupProperties

/** One tab as the pickers show it. */
internal data class TabEntry(val id: String,val title: String,val detail: String,val programs: List<ProgramIdentity>,
    val selectedPane: Int,val attention: Boolean,val working: Boolean)

/** The Activity sheet's "blocked" amber: the one warm color, reserved for what needs you. */
@Composable internal fun attentionColor()=if(LocalTerminalPalette.current.dark) Color(0xffe6a947) else Color(0xffb77710)

/**
 * A tab's status, ranked like the Activity sheet: needs you is warm and labelled (you must act);
 * working is a quiet pulsing dot (nothing to do); idle shows nothing. The pulse is read only while
 * drawing, so it never recomposes the row.
 */
@Composable internal fun TabStatus(tab: TabEntry,label: Boolean=false) {
    when {
        tab.attention -> Row(Modifier.semantics(mergeDescendants=true) {contentDescription="Needs you"},
            verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(6.dp)) {
            val amber=attentionColor()
            Box(Modifier.size(8.dp).background(amber,CircleShape))
            if(label) Text("Needs you",fontSize=11.sp,fontWeight=FontWeight.Medium,color=amber,maxLines=1)
        }
        tab.working -> {
            val phase=rememberWorkingPhase(true)
            val color=MaterialTheme.colorScheme.primary
            Box(Modifier.size(8.dp).semantics {contentDescription="Working"}.drawBehind {
                // Same pulse as the Activity sheet's working bars (45–100%), so it never fades out.
                val brightness=(.725-.275*kotlin.math.cos(phase.value*2*Math.PI)).toFloat()
                drawCircle(color.copy(alpha=brightness))
            })
        }
    }
}

/** Tap the header title: the session's tabs in one card, then New tab beside the two splits, and Switch session. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable internal fun TabPickerCard(server: String,session: String,tabs: List<TabEntry>,current: String,enabled: Boolean,canCreate: Boolean,canSplit: Boolean,
    onSelect: (String)->Unit,onNewTab: ()->Unit,onSplit: (PaneSplit)->Unit,onSessions: ()->Unit,onClose: ()->Unit,onManage: ()->Unit,hazeState: dev.chrisbanes.haze.HazeState?=null) {
    val card=RoundedCornerShape(30.dp)
    ModalBottomSheet(onDismissRequest=onClose,sheetState=rememberModalBottomSheetState(skipPartiallyExpanded=true),containerColor=Color.Transparent,
        contentColor=MaterialTheme.colorScheme.onSurface,tonalElevation=0.dp,dragHandle=null,contentWindowInsets={WindowInsets(0,0,0,0)}) {
        Column(Modifier.fillMaxWidth().windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom)).padding(horizontal=10.dp).padding(bottom=10.dp)
            .drawerFrost(hazeState,card).padding(14.dp),verticalArrangement=Arrangement.spacedBy(10.dp)) {
            Row(Modifier.fillMaxWidth().padding(start=8.dp),verticalAlignment=Alignment.CenterVertically) {
                Column(Modifier.weight(1f)) {
                    Text(server,fontSize=12.sp,fontWeight=FontWeight.Medium,color=MaterialTheme.colorScheme.onSurfaceVariant,maxLines=1,overflow=TextOverflow.Ellipsis)
                    Text(session,fontSize=20.sp,fontWeight=FontWeight.Bold,maxLines=1,overflow=TextOverflow.Ellipsis)
                }
                // Reorder, rename, close and float live in the full switcher; the split icon belongs to the split buttons below.
                Box(Modifier.size(44.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha=.6f))
                    .clickable(enabled=enabled,role=Role.Button,onClick=onManage).semantics {contentDescription="Tabs and panes"},contentAlignment=Alignment.Center) {
                    ChromeSymbol(ChromeIcon.EDIT,Modifier.size(18.dp))
                }
                Spacer(Modifier.width(8.dp))
                Box(Modifier.size(44.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha=.6f))
                    .clickable(role=Role.Button,onClick=onClose).semantics {contentDescription="Close tabs"},contentAlignment=Alignment.Center) {
                    ChromeSymbol(ChromeIcon.CLOSE,Modifier.size(18.dp))
                }
            }
            LazyColumn(Modifier.fillMaxWidth().heightIn(max=sheetBodyMax(420.dp,chrome=300.dp)).clip(RoundedCornerShape(22.dp))
                .background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha=.55f)).semantics {contentDescription="Tabs"}) {
                itemsIndexed(tabs,key={_,tab->tab.id}) {index,tab->
                    val active=tab.id==current
                    Row(Modifier.fillMaxWidth().heightIn(min=60.dp).clickable(enabled=enabled,role=Role.Button) {onSelect(tab.id)}
                        .semantics {selected=active;stateDescription=if(active) "Current tab" else ""}.padding(horizontal=14.dp,vertical=8.dp),
                        verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(12.dp)) {
                        Box(Modifier.width(48.dp)) {ProgramStack(tab.programs,tab.selectedPane,framed=true,fixedSlot=true)}
                        Column(Modifier.weight(1f)) {
                            Text(tab.title,fontSize=16.sp,fontWeight=FontWeight.SemiBold,maxLines=1,overflow=TextOverflow.Ellipsis)
                            if(tab.detail.isNotBlank()) Text(tab.detail,fontSize=12.sp,color=MaterialTheme.colorScheme.onSurfaceVariant,maxLines=1,overflow=TextOverflow.Ellipsis,fontFamily=MonoUi)
                        }
                        TabStatus(tab,label=true)
                        Box(Modifier.size(22.dp),contentAlignment=Alignment.Center) {if(active) ChromeSymbol(ChromeIcon.CHECK,Modifier.size(20.dp),MaterialTheme.colorScheme.primary)}
                    }
                    if(index<tabs.lastIndex) HorizontalDivider(Modifier.padding(start=74.dp,end=14.dp),color=MaterialTheme.colorScheme.outline.copy(alpha=.25f))
                }
            }
            Row(Modifier.fillMaxWidth(),horizontalArrangement=Arrangement.spacedBy(8.dp)) {
                PickerPill(ChromeIcon.ADD,"New tab",canCreate,Modifier.weight(1f),onClick=onNewTab)
                PaneSplit.entries.forEach {split->SplitButton(split,canSplit,54.dp,RoundedCornerShape(27.dp)) {onSplit(split)}}
            }
            PickerPill(ChromeIcon.SESSIONS,"Switch session",enabled,chevron=true,onClick=onSessions)
        }
    }
}

/** Split the current tab's shown pane; the drawing is the label. */
@Composable internal fun SplitButton(split: PaneSplit,enabled: Boolean,size: androidx.compose.ui.unit.Dp,shape: androidx.compose.ui.graphics.Shape,onClick: ()->Unit) {
    Box(Modifier.size(size).clip(shape).background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha=.55f))
        .clickable(enabled=enabled,role=Role.Button,onClick=onClick).semantics {contentDescription=split.label},contentAlignment=Alignment.Center) {
        SplitDiagram(split,Modifier.size(22.dp),MaterialTheme.colorScheme.onSurface.copy(alpha=if(enabled) 1f else .38f))
    }
}

@Composable private fun PickerPill(icon: ChromeIcon,label: String,enabled: Boolean,modifier: Modifier=Modifier.fillMaxWidth(),chevron: Boolean=false,onClick: ()->Unit) {
    Row(modifier.height(54.dp).clip(RoundedCornerShape(27.dp)).background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha=.55f))
        .clickable(enabled=enabled,role=Role.Button,onClick=onClick).padding(horizontal=18.dp),
        verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(14.dp)) {
        val color=MaterialTheme.colorScheme.onSurface.copy(alpha=if(enabled) 1f else .38f)
        ChromeSymbol(icon,Modifier.size(20.dp),color)
        Text(label,Modifier.weight(1f),fontSize=16.sp,fontWeight=FontWeight.Medium,color=color)
        if(chevron) ChromeSymbol(ChromeIcon.CHEVRON,Modifier.size(16.dp).graphicsLayer {rotationZ=-90f},MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/** Press-and-hold on the title: where the finger is, and where the list sits, in root pixels. */
internal class TabHoldState {
    var open by mutableStateOf(false)
        private set
    var pointerY by mutableFloatStateOf(0f)
        private set
    var listTop=0f
    var rowHeight=1f
    fun start(y: Float) {pointerY=y;open=true}
    fun move(y: Float) {pointerY=y}
    fun finish() {open=false}
    /** The row under the finger, or -1 above/below the list. */
    fun index(count: Int): Int {
        if(!open || count==0 || pointerY<listTop) return -1
        return ((pointerY-listTop)/rowHeight).toInt().takeIf {it<count} ?: -1
    }
}

/** The hold-and-slide list: shown under the header while the title is held; release on a row to switch. */
@Composable internal fun TabHoldList(state: TabHoldState,tabs: List<TabEntry>,current: String,headerBottom: Int,listTopInRoot: Float,hazeState: dev.chrisbanes.haze.HazeState?=null) {
    val density=androidx.compose.ui.platform.LocalDensity.current
    val haptics=androidx.compose.ui.platform.LocalHapticFeedback.current
    val rowHeight=with(density) {48.dp.toPx()}
    val padding=with(density) {6.dp.toPx()}
    state.rowHeight=rowHeight
    state.listTop=listTopInRoot+padding
    val highlighted=state.index(tabs.size)
    LaunchedEffect(highlighted) {if(highlighted>=0) haptics.performHapticFeedback(androidx.compose.ui.hapticfeedback.HapticFeedbackType.TextHandleMove)}
    // Not focusable: the finger that opened it keeps driving the header's gesture.
    Popup(alignment=Alignment.TopStart,offset=IntOffset(with(density) {56.dp.roundToPx()},headerBottom),properties=PopupProperties(focusable=false)) {
        // A popup is its own window, so the workspace blur cannot reach it: use an opaque surface.
        Column(Modifier.width(260.dp).shadow(16.dp,RoundedCornerShape(22.dp)).clip(RoundedCornerShape(22.dp))
            .background(MaterialTheme.colorScheme.surfaceContainerHigh).border(1.dp,MaterialTheme.colorScheme.outline.copy(alpha=.25f),RoundedCornerShape(22.dp))
            .padding(vertical=6.dp).semantics {contentDescription="Choose a tab"}) {
            tabs.forEachIndexed {index,tab->
                val active=tab.id==current
                Row(Modifier.fillMaxWidth().height(48.dp).padding(horizontal=6.dp).clip(RoundedCornerShape(14.dp))
                    .background(if(index==highlighted) MaterialTheme.colorScheme.primary.copy(alpha=.28f) else Color.Transparent)
                    .border(if(index==highlighted) 1.dp else 0.dp,if(index==highlighted) MaterialTheme.colorScheme.primary.copy(alpha=.6f) else Color.Transparent,RoundedCornerShape(14.dp))
                    .padding(horizontal=10.dp),verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(10.dp)) {
                    Box(Modifier.width(44.dp)) {ProgramStack(tab.programs,tab.selectedPane,framed=true,fixedSlot=true)}
                    Text(tab.title,Modifier.weight(1f),fontSize=15.sp,fontWeight=FontWeight.SemiBold,maxLines=1,overflow=TextOverflow.Ellipsis)
                    TabStatus(tab)
                    Box(Modifier.size(18.dp),contentAlignment=Alignment.Center) {if(active) ChromeSymbol(ChromeIcon.CHECK,Modifier.size(16.dp),MaterialTheme.colorScheme.primary)}
                }
            }
        }
    }
}
