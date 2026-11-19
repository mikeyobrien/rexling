package dev.shulhi.rex

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.Popup
import androidx.compose.ui.window.PopupProperties

/** The Activity sheet's "blocked" amber: the one warm color, reserved for what needs you. */
@Composable internal fun attentionColor()=if(LocalTerminalPalette.current.dark) Color(0xffe6a947) else Color(0xffb77710)

/** Red is for what ends a program: Close, and the question it asks first. */
@Composable internal fun destructiveColor()=if(LocalTerminalPalette.current.dark) Color(0xfff06868) else Color(0xffcf3d3d)

/**
 * A row's status, ranked like the Activity sheet: needs you is warm and labelled (you must act);
 * working is a quiet dot (nothing to do); idle shows nothing. The dot pulses once when the status
 * changes and then rests, so a list left on screen draws nothing while work goes on.
 */
@Composable internal fun RowStatus(attention: Boolean,working: Boolean,label: Boolean=false) {
    val status=if(attention) 2 else if(working) 1 else 0
    val pulse=remember {Animatable(1f)}
    var shown by remember {mutableIntStateOf(status)}
    LaunchedEffect(status) {
        if(status!=shown) {shown=status;if(status!=0) {pulse.snapTo(1.9f);pulse.animateTo(1f,spring(dampingRatio=.4f,stiffness=300f))}}
    }
    val dot=Modifier.size(8.dp).graphicsLayer {scaleX=pulse.value;scaleY=pulse.value}
    when {
        attention -> Row(Modifier.semantics(mergeDescendants=true) {contentDescription="Needs you"},
            verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(6.dp)) {
            val amber=attentionColor()
            Box(dot.background(amber,CircleShape))
            if(label) Text("Needs you",fontSize=11.sp,fontWeight=FontWeight.Medium,color=amber,maxLines=1)
        }
        working -> Box(dot.background(MaterialTheme.colorScheme.primary,CircleShape).semantics {contentDescription="Working"})
    }
}

@Composable internal fun TabStatus(tab: TabEntry,label: Boolean=false)=RowStatus(tab.attention,tab.working,label)

/** Split the current tab's shown pane; the drawing is the label. */
@Composable internal fun SplitButton(split: PaneSplit,enabled: Boolean,size: androidx.compose.ui.unit.Dp,shape: androidx.compose.ui.graphics.Shape,onClick: ()->Unit) {
    Box(Modifier.size(size).clip(shape).background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha=.55f))
        .clickable(enabled=enabled,role=Role.Button,onClick=onClick).semantics {contentDescription=split.label},contentAlignment=Alignment.Center) {
        SplitDiagram(split,Modifier.size(22.dp),MaterialTheme.colorScheme.onSurface.copy(alpha=if(enabled) 1f else .38f))
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
        Column(Modifier.width(260.dp).shadow(16.dp,Kit.group).clip(Kit.group)
            .background(MaterialTheme.colorScheme.surfaceContainerHigh).border(1.dp,MaterialTheme.colorScheme.outline.copy(alpha=.25f),Kit.group)
            .padding(vertical=6.dp).semantics {contentDescription="Choose a tab"}) {
            tabs.forEachIndexed {index,tab->
                val active=tab.id==current
                Row(Modifier.fillMaxWidth().height(48.dp).padding(horizontal=6.dp).clip(Kit.row)
                    .background(if(index==highlighted) MaterialTheme.colorScheme.primary.copy(alpha=.28f) else Color.Transparent)
                    .border(if(index==highlighted) 1.dp else 0.dp,if(index==highlighted) MaterialTheme.colorScheme.primary.copy(alpha=.6f) else Color.Transparent,Kit.row)
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
