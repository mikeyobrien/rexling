package dev.shulhi.rex

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.foundation.gestures.scrollBy
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListScope
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.RoundRect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.zIndex
import kotlinx.coroutines.delay
import kotlin.math.abs
import kotlin.math.roundToInt

/** How far a row slides to show its Close button. */
private val RevealWidth=84.dp
private val RowShape=RoundedCornerShape(14.dp)

/**
 * A list whose rows are managed in place. Holding a row opens its menu; dragging the held row
 * reorders it among its [siblings]. Swipes and the in-row confirmation belong to [ManagedRow].
 */
@Composable internal fun ManagedList(rows: RowsState,list: LazyListState,enabled: Boolean,actionsFor: (String)->List<RowAction>,
    siblings: (String)->List<String>,onMove: (String,String?)->Unit,order: List<String>,modifier: Modifier=Modifier,
    contentPadding: PaddingValues=PaddingValues(0.dp),content: LazyListScope.()->Unit) {
    val haptic=LocalHapticFeedback.current
    val edge=with(LocalDensity.current) {32.dp.toPx()}
    val speed=with(LocalDensity.current) {420.dp.toPx()}
    var bounds by remember {mutableStateOf<LayoutCoordinates?>(null)}
    // Panes fold away while their tab is carried. Holding the list's height keeps its top where it
    // was, so the row in hand stays under the finger instead of following a shrinking card.
    var heldHeight by remember {mutableStateOf<Dp?>(null)}
    val density=LocalDensity.current
    val latestActions by rememberUpdatedState(actionsFor)
    val latestSiblings by rememberUpdatedState(siblings)
    val latestMove by rememberUpdatedState(onMove)
    val drag=rows.drag
    // A row the server added, removed or moved while it was held would drop in the wrong place.
    LaunchedEffect(order,enabled) {drag.cancel();rows.retain(order.toSet())}
    // If the server keeps a row we asked it to close, give the row back.
    LaunchedEffect(rows.closing) {if(rows.closing.isNotEmpty()) {delay(4000);rows.closing=emptySet()}}
    LaunchedEffect(drag.id) {
        var previous=withFrameNanos {it}
        while(drag.id!=null) {
            val now=withFrameNanos {it};val dt=((now-previous)/1_000_000_000f).coerceAtMost(.05f);previous=now
            if(!drag.moved) continue
            val end=list.layoutInfo.viewportEndOffset.toFloat()
            val velocity=when {
                drag.pointer<edge -> -((edge-drag.pointer)/edge).coerceIn(0f,1f)
                drag.pointer>end-edge -> ((drag.pointer-end+edge)/edge).coerceIn(0f,1f)
                else -> 0f
            }
            if(velocity!=0f) {list.scrollBy(velocity*speed*dt);drag.cross(velocity,list)}
        }
    }
    LazyColumn(modifier.then(heldHeight?.takeIf {drag.id!=null}?.let {Modifier.height(it)} ?: Modifier).onGloballyPositioned {bounds=it}.pointerInput(enabled) {
        if(enabled) detectTabDrag(
            start={point->
                // Item offsets start after the content padding; the pointer starts at the list's edge.
                val shift=list.layoutInfo.viewportStartOffset
                val y=point.y+shift
                val item=list.layoutInfo.visibleItemsInfo.firstOrNull {y>=it.offset && y<it.offset+it.size}
                val key=item?.key as? String
                if(item==null || key==null || latestActions(key).none {it.menu}) false else {
                    val group=latestSiblings(key)
                    if(group.size>1 && drag.start(group,list,y)) heldHeight=with(density) {size.height.toDp()}
                    val origin=bounds?.takeIf {it.isAttached}?.localToRoot(Offset(0f,(item.offset-shift).toFloat())) ?: Offset.Zero
                    rows.revealed=null;rows.confirming=null
                    rows.menu=RowMenu(key,origin.y,origin.y+item.size,origin.x+size.width)
                    haptic.performHapticFeedback(HapticFeedbackType.LongPress)
                    true
                }
            },
            move={amount->
                drag.move(amount.y,viewConfiguration.touchSlop,list)
                // Moving the held row means reordering, not choosing from the menu.
                if(drag.moved) rows.menu=null
            },
            end={
                val id=drag.id
                if(id!=null && drag.changed()) latestMove(id,drag.order.getOrNull(drag.order.indexOf(id)+1))
                drag.cancel()
            },cancel={drag.cancel()})
    },state=list,userScrollEnabled=drag.id==null && rows.menu==null,contentPadding=contentPadding,content=content)
}

/**
 * The one row: tap opens it, a sideways swipe shows Close, and closing something that runs a
 * program asks here instead of in a dialog. [lift] follows the finger while the row is dragged.
 */
@Composable internal fun ManagedRow(rows: RowsState,key: String,selected: Boolean,enabled: Boolean,close: RowClose?,actions: List<RowAction>,
    onOpen: ()->Unit,modifier: Modifier=Modifier,minHeight: Dp=56.dp,lift: (()->Float)?=null,content: @Composable RowScope.()->Unit) {
    val scheme=MaterialTheme.colorScheme
    @Suppress("NAME_SHADOWING") val modifier=modifier.arriving(remember(key) {rows.arrival(key)})
    val distance=with(LocalDensity.current) {RevealWidth.toPx()}
    val side=rows.revealed?.takeIf {it.first==key}?.second ?: 0
    val reveal=remember {Animatable(0f)}
    var position by remember {mutableFloatStateOf(0f)}
    var dragging by remember {mutableStateOf(false)}
    LaunchedEffect(side,dragging) {if(!dragging) reveal.animateTo(side*distance,spring(dampingRatio=.9f,stiffness=600f))}
    // Read while placing and drawing, so a swipe never recomposes the row.
    val slid={if(dragging) position else reveal.value}
    // Which side's Close the row has uncovered: 1 leading, -1 trailing, 0 neither.
    val uncovered by remember {derivedStateOf {slid().let {if(it>0f) 1 else if(it<0f) -1 else 0}}}
    val busy=key in rows.closing
    val held=rows.menu?.key==key
    val lifted=rows.drag.id==key && rows.drag.moved
    val red=destructiveColor()
    val inset=4.dp
    Box(modifier.fillMaxWidth().heightIn(min=minHeight).graphicsLayer {alpha=if(busy) .4f else 1f}.drawBehind {
        // The slot the lifted row will land in.
        if(lifted) drawRoundRect(scheme.primary.copy(alpha=.45f),Offset(inset.toPx(),3.dp.toPx()),
            Size(size.width-inset.toPx()*2,size.height-6.dp.toPx()),CornerRadius(14.dp.toPx()),style=Stroke(1.dp.toPx()))
    }) {
        if(close!=null && rows.confirming==key) ConfirmClose(close,enabled && !busy,minHeight,onCancel={rows.confirming=null}) {rows.confirmClose(key,close)}
        else {
        if(close!=null && uncovered!=0) {
            val leading=uncovered>0
            // Only as much of the button as the row has uncovered.
            Box(Modifier.align(if(leading) Alignment.CenterStart else Alignment.CenterEnd).width(RevealWidth).height(minHeight).drawWithContent {
                val shown=(if(leading) slid() else -slid()).coerceAtMost(size.width)
                if(shown>0f) clipRect(if(leading) 0f else size.width-shown,0f,if(leading) shown else size.width,size.height) {this@drawWithContent.drawContent()}
            }.padding(horizontal=5.dp,vertical=6.dp).clip(RowShape).background(red)
                .clickable(enabled=enabled && !busy && side==(if(leading) 1 else -1),role=Role.Button) {rows.requestClose(key,close)}
                .semantics {contentDescription=close.question.removeSuffix("?")},contentAlignment=Alignment.Center) {
                ChromeSymbol(ChromeIcon.CLOSE,Modifier.size(20.dp),Color.White)
            }
        }
        Row(Modifier.fillMaxWidth().heightIn(min=minHeight)
            .then(if(lift!=null) Modifier.graphicsLayer {translationY=lift()} else Modifier)
            .offset {IntOffset(slid().roundToInt(),0)}
            .padding(horizontal=inset,vertical=3.dp)
            .then(when {
                lifted->Modifier.shadow(12.dp,RowShape).background(scheme.surfaceContainerHighest,RowShape).border(1.dp,scheme.primary.copy(alpha=.65f),RowShape)
                held->Modifier.background(scheme.onSurface.copy(alpha=.08f),RowShape)
                selected->Modifier.background(scheme.primary.copy(alpha=.14f),RowShape)
                else->Modifier
            }).clip(RowShape)
            .draggable(rememberDraggableState {delta->position=(position+delta).coerceIn(-distance,distance)},Orientation.Horizontal,
                enabled=enabled && close!=null && !busy,
                onDragStarted={
                    position=reveal.value;dragging=true
                    rows.menu=null;rows.confirming=null
                    if(rows.revealed?.first!=key) rows.revealed=null
                },onDragStopped={
                    reveal.snapTo(position)
                    rows.revealed=if(abs(position)>distance*.35f) key to (if(position>0) 1 else -1) else null
                    dragging=false
                })
            .clickable(enabled=enabled && !busy,role=Role.Button) {if(side!=0) rows.revealed=null else onOpen()}
            .semantics {
                this.selected=selected
                customActions=actions.filter {it.enabled}.map {action->CustomAccessibilityAction(action.label) {action.run();true}}
            },verticalAlignment=Alignment.CenterVertically) {
            Box(Modifier.width(3.dp).height(28.dp).background(if(selected && !lifted) scheme.primary else Color.Transparent,RoundedCornerShape(1.5.dp)))
            content()
            if(lifted) Grip()
        }
        }
    }
}

/** Six dots: this row is in your hand. */
@Composable private fun Grip() {
    val color=MaterialTheme.colorScheme.onSurfaceVariant
    Canvas(Modifier.padding(end=12.dp).size(9.dp,15.dp)) {
        for(x in 0..1) for(y in 0..2) drawCircle(color,1.3.dp.toPx(),Offset(size.width*(.15f+.7f*x),size.height*(.1f+.4f*y)))
    }
}

@Composable private fun ConfirmClose(close: RowClose,enabled: Boolean,minHeight: Dp,onCancel: ()->Unit,onConfirm: ()->Unit) {
    val red=destructiveColor()
    Row(Modifier.fillMaxWidth().heightIn(min=minHeight).padding(horizontal=4.dp,vertical=3.dp).background(red.copy(alpha=.14f),RowShape)
        .padding(start=16.dp,end=6.dp),verticalAlignment=Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text(close.question,fontSize=15.sp,lineHeight=20.sp,fontWeight=FontWeight.SemiBold,maxLines=1,overflow=TextOverflow.Ellipsis)
            Text(close.detail,fontSize=12.sp,lineHeight=17.sp,fontFamily=MonoUi,color=MaterialTheme.colorScheme.onSurfaceVariant,maxLines=1,overflow=TextOverflow.Ellipsis)
        }
        Box(Modifier.heightIn(min=40.dp).clip(CircleShape).clickable(role=Role.Button,onClick=onCancel).padding(horizontal=12.dp),contentAlignment=Alignment.Center) {
            Text("Cancel",fontSize=14.sp,fontWeight=FontWeight.Medium,color=MaterialTheme.colorScheme.onSurfaceVariant)
        }
        Box(Modifier.heightIn(min=38.dp).clip(CircleShape).background(red).clickable(enabled=enabled,role=Role.Button,onClick=onConfirm).padding(horizontal=18.dp),
            contentAlignment=Alignment.Center) {
            Text("Close",fontSize=14.sp,fontWeight=FontWeight.SemiBold,color=Color.White)
        }
    }
}

/**
 * The held row's menu, drawn over the list's container so it can leave the list's bounds: above
 * the row when there is room, below it otherwise. Put it last in that container's Box.
 */
@Composable internal fun BoxScope.RowMenuHost(rows: RowsState,actionsFor: (String)->List<RowAction>) {
    val menu=rows.menu ?: return
    val items=actionsFor(menu.key).filter {it.menu}
    if(items.isEmpty()) return
    Box(Modifier.matchParentSize().pointerInput(Unit) {
        awaitEachGesture {awaitFirstDown(requireUnconsumed=false).consume();rows.menu=null}
    })
    Layout({RowMenuCard(items) {rows.menu=null}},Modifier.matchParentSize()) {measurables,constraints->
        val card=measurables.first().measure(Constraints(maxWidth=constraints.maxWidth,maxHeight=constraints.maxHeight))
        layout(constraints.maxWidth,constraints.maxHeight) {
            val origin=coordinates?.positionInRoot() ?: Offset.Zero
            val gap=6.dp.roundToPx();val margin=8.dp.roundToPx()
            val above=(menu.top-origin.y).roundToInt()-gap-card.height
            val below=(menu.bottom-origin.y).roundToInt()+gap
            val y=(if(above>=margin) above else below).coerceIn(margin,(constraints.maxHeight-card.height-margin).coerceAtLeast(margin))
            val x=((menu.right-origin.x).roundToInt()-margin-card.width).coerceIn(margin,(constraints.maxWidth-card.width-margin).coerceAtLeast(margin))
            card.place(x,y)
        }
    }
}

@Composable private fun RowMenuCard(items: List<RowAction>,dismiss: ()->Unit) {
    val scheme=MaterialTheme.colorScheme
    val shape=RoundedCornerShape(20.dp)
    val appear=remember {Animatable(0f)}
    LaunchedEffect(Unit) {appear.animateTo(1f,tween(120))}
    Column(Modifier.width(206.dp).graphicsLayer {
        alpha=appear.value;scaleX=.94f+.06f*appear.value;scaleY=scaleX;transformOrigin=TransformOrigin(1f,1f)
    }.shadow(16.dp,shape).clip(shape).background(scheme.surfaceContainerHigh).border(1.dp,scheme.outline.copy(alpha=.3f),shape)
        // A tap on the card's own padding is not a tap outside it.
        .pointerInput(Unit) {detectTapGestures {}}.padding(vertical=6.dp).semantics {contentDescription="Row actions"}) {
        items.forEach {item->
            val color=(if(item.destructive) destructiveColor() else scheme.onSurface).copy(alpha=if(item.enabled) 1f else .38f)
            Row(Modifier.fillMaxWidth().height(44.dp).clickable(enabled=item.enabled,role=Role.Button) {dismiss();item.run()}.padding(horizontal=16.dp),
                verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(14.dp)) {
                if(item.split!=null) SplitDiagram(item.split,Modifier.size(20.dp),color) else item.icon?.let {ChromeSymbol(it,Modifier.size(20.dp),color)}
                Text(item.label,fontSize=15.sp,fontWeight=FontWeight.Medium,color=color)
            }
        }
    }
}

/** Where a pane sits in its tab: the tab's layout with this pane filled in. */
@Composable internal fun PaneThumb(layout: List<PanePlacement>,id: String,modifier: Modifier=Modifier) {
    val scheme=MaterialTheme.colorScheme
    Canvas(modifier.size(30.dp,22.dp)) {
        val stroke=1.2.dp.toPx()
        val frame=Rect(Offset(stroke/2,stroke/2),Size(size.width-stroke,size.height-stroke))
        val radius=CornerRadius(4.5.dp.toPx())
        clipPath(Path().apply {addRoundRect(RoundRect(frame,radius))}) {
            layout.forEach {pane->
                val origin=Offset(frame.left+pane.x*frame.width,frame.top+pane.y*frame.height)
                val area=Size(pane.width*frame.width,pane.height*frame.height)
                if(pane.id==id) drawRect(scheme.primary.copy(alpha=.5f),origin,area)
                drawRect(scheme.onSurfaceVariant,origin,area,style=Stroke(stroke*.8f))
            }
        }
        drawRoundRect(scheme.onSurfaceVariant,frame.topLeft,frame.size,radius,style=Stroke(stroke))
    }
}

@Composable private fun RowDivider() {
    HorizontalDivider(Modifier.padding(start=68.dp,end=16.dp),color=MaterialTheme.colorScheme.outline.copy(alpha=.25f))
}

@Composable private fun RowText(title: String,detail: String,modifier: Modifier) {
    Column(modifier) {
        Text(title,fontSize=15.sp,lineHeight=20.sp,fontWeight=FontWeight.SemiBold,maxLines=1,overflow=TextOverflow.Ellipsis)
        if(informative(detail)) Text(detail,fontSize=12.sp,lineHeight=17.sp,fontFamily=MonoUi,color=MaterialTheme.colorScheme.onSurfaceVariant,maxLines=1,overflow=TextOverflow.Ellipsis)
    }
}

@Composable private fun PanesToggle(open: Boolean,onClick: ()->Unit) {
    val turn by animateFloatAsState(if(open) 180f else 0f,label="Panes chevron")
    Box(Modifier.size(40.dp).clip(CircleShape).clickable(role=Role.Button,onClick=onClick)
        .semantics {contentDescription=if(open) "Hide panes" else "Show panes"},contentAlignment=Alignment.Center) {
        ChromeSymbol(ChromeIcon.CHEVRON,Modifier.size(16.dp).graphicsLayer {rotationZ=turn},MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

/**
 * The session's tabs. A tab with several panes opens in place to list them; while it is open the
 * highlight moves from the tab to the pane being viewed.
 */
internal fun LazyListScope.tabRows(rows: RowsState,list: LazyListState,model: SwitcherModel,manager: RowManager,dividers: Boolean=true) {
    val drag=rows.drag
    val held=drag.id.orEmpty()
    val tabs=if(held.startsWith("tab:")) drag.order.mapNotNull {key->model.tabs.firstOrNull {tabKey(it.id)==key}} else model.tabs
    tabs.forEachIndexed {index,tab->
        val key=tabKey(tab.id)
        // Panes fold away while their tab is carried, so it moves as one row.
        val open=tab.panes.size>1 && key in rows.expanded && !(drag.moved && held.startsWith("tab:"))
        val current=tab.id==model.currentTab
        item(key=key,contentType="tab") {
            Column(Modifier.animateItem(placementSpec=if(held==key) null else spring()).zIndex(if(held==key) 1f else 0f)) {
                ManagedRow(rows,key,current && !open,model.enabled,manager.close(key),manager.actions(key),{model.openTab(tab.id)},lift={drag.translation(key,list)}) {
                    Box(Modifier.padding(start=5.dp).width(48.dp)) {ProgramStack(tab.programs,tab.selectedPane,framed=true,fixedSlot=true)}
                    RowText(tab.title,tab.detail,Modifier.weight(1f).padding(start=8.dp,end=8.dp))
                    RowStatus(tab.attention,tab.working,label=true)
                    if(tab.panes.size>1) PanesToggle(open) {rows.expanded=if(key in rows.expanded) rows.expanded-key else rows.expanded+key}
                    else Spacer(Modifier.width(12.dp))
                }
                if(dividers && !open && index<tabs.lastIndex) RowDivider()
            }
        }
        if(open) {
            val panes=if(held.startsWith("pane:") && tab.panes.any {paneKey(it.id)==held}) drag.order.mapNotNull {k->tab.panes.firstOrNull {paneKey(it.id)==k}} else tab.panes
            panes.forEachIndexed {paneIndex,pane->
                val row=paneKey(pane.id)
                item(key=row,contentType="pane") {
                    Column(Modifier.animateItem(placementSpec=if(held==row) null else spring()).zIndex(if(held==row) 1f else 0f)) {
                        ManagedRow(rows,row,current && pane.id==model.currentPane,model.enabled,manager.close(row),manager.actions(row),{model.openPane(pane.id)},
                            minHeight=46.dp,lift={drag.translation(row,list)}) {
                            Box(Modifier.padding(start=61.dp).width(30.dp),contentAlignment=Alignment.Center) {
                                if(tab.layout.any {it.id==pane.id}) PaneThumb(tab.layout,pane.id) else PaneProgramTile(pane.program,Modifier.size(26.dp,21.dp),false,0f)
                            }
                            val dim=MaterialTheme.colorScheme.onSurfaceVariant
                            // One line, so a long title gives way to the folder rather than the other way round.
                            Text(buildAnnotatedString {
                                append(pane.title)
                                if(informative(pane.detail)) withStyle(SpanStyle(fontSize=12.sp,fontWeight=FontWeight.Normal,fontFamily=MonoUi,color=dim)) {append("  ${pane.detail}")}
                            },Modifier.weight(1f).padding(start=12.dp,end=8.dp),fontSize=14.sp,fontWeight=FontWeight.Medium,maxLines=1,overflow=TextOverflow.Ellipsis)
                            RowStatus(false,pane.working)
                            Spacer(Modifier.width(14.dp))
                        }
                        if(dividers && paneIndex==panes.lastIndex && index<tabs.lastIndex) RowDivider()
                    }
                }
            }
        }
    }
}

/** Sessions, in the order given. Opening the one you are in is up to [onOpen]. */
internal fun LazyListScope.sessionRows(rows: RowsState,sessions: List<SessionEntry>,model: SwitcherModel,manager: RowManager,onOpen: (SessionEntry)->Unit,
    dividers: Boolean=true) {
    sessions.forEachIndexed {index,entry->
        val key=sessionRowKey(entry.key)
        item(key=key,contentType="session") {
            Column(Modifier.animateItem()) {
                val busy=model.busySession==entry.key
                ManagedRow(rows,key,entry.key==model.currentSession,!busy,manager.close(key),manager.actions(key),{onOpen(entry)}) {
                    Box(Modifier.padding(start=5.dp).width(48.dp),contentAlignment=Alignment.Center) {
                        ChromeSymbol(ChromeIcon.SESSIONS,Modifier.size(20.dp),MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    RowText(entry.target.session.label,entry.detail,Modifier.weight(1f).padding(start=8.dp,end=8.dp))
                    if(busy) CircularProgressIndicator(Modifier.size(15.dp),strokeWidth=1.5.dp) else RowStatus(entry.attention,entry.working,label=true)
                    Spacer(Modifier.width(12.dp))
                }
                if(dividers && index<sessions.lastIndex) RowDivider()
            }
        }
    }
}

/** The rename and dock dialogs that row menus open. */
@Composable internal fun RowDialogs(rows: RowsState,model: SwitcherModel) {
    rows.rename?.let {target->
        val available=model.tabs.firstOrNull {it.id==target.window}?.let {target.tab || it.panes.any {pane->pane.id==target.id}}==true
        TerminalRenameDialog(target,model.ready && available,onSave={label->
            if(target.tab) model.renameTab(target.id,label) else model.renamePane(target.window,target.id,label)
        },onClose={rows.rename=null})
    }
    rows.dock?.let {(window,id)->
        val tab=model.tabs.firstOrNull {it.id==window}
        val anchors=tab?.panes.orEmpty().filter {!it.floating && it.id!=id}
        DockPaneDialog(anchors.map {BlockItem(it.id,it.title)},anchors.associate {it.id to it.program},emptyMap(),
            model.ready && tab?.panes?.any {it.id==id && it.floating}==true,onDock={anchor,split->model.dockPane(window,id,anchor,split)},onClose={rows.dock=null})
    }
    rows.renameSession?.let {target->
        SessionRenameDialog(target,model.busySession.isBlank(),onSave={label->if(model.renameSession(target,label)) rows.renameSession=null},onDismiss={rows.renameSession=null})
    }
}
