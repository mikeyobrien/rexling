package dev.shulhi.rex

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import dev.chrisbanes.haze.HazeState

/**
 * Where sheets open on a wide screen. A bottom sheet there floats over both the tab list and the
 * terminal; docked, it takes the terminal side only, so the list stays in reach.
 */
@Stable internal class SheetDock {
    /** True while the wide layout is showing a place to dock into. */
    var active by mutableStateOf(false)
    val entries=mutableStateListOf<DockEntry>()
}
internal class DockEntry(val card: Boolean,val onClose: State<()->Unit>,val content: State<@Composable ()->Unit>)
internal val LocalSheetDock=staticCompositionLocalOf<SheetDock?> {null}

/** True where a sheet would dock instead of rising from the bottom. */
@Composable internal fun sheetDocks()=LocalSheetDock.current?.active==true

/**
 * The window a sheet lives in: a bottom sheet on a phone, the terminal side on a wide screen.
 * A [card] floats at the bottom of its side; otherwise the sheet fills it as a panel.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable internal fun SheetWindow(onClose: ()->Unit,card: Boolean=false,content: @Composable ()->Unit) {
    val dock=LocalSheetDock.current?.takeIf {it.active}
    if(dock==null) ModalBottomSheet(onDismissRequest=onClose,sheetState=rememberModalBottomSheetState(skipPartiallyExpanded=true),containerColor=Color.Transparent,
        contentColor=MaterialTheme.colorScheme.onSurface,tonalElevation=0.dp,dragHandle=null,contentWindowInsets={WindowInsets(0,0,0,0)}) {content()}
    else {
        val close=rememberUpdatedState(onClose);val body=rememberUpdatedState(content)
        DisposableEffect(dock) {
            val entry=DockEntry(card,close,body)
            dock.entries+=entry
            onDispose {dock.entries-=entry}
        }
    }
}

/** A sheet's surface: frosted glass over the app as a bottom sheet, the card's own surface when docked. */
@Composable internal fun Modifier.sheetSurface(hazeState: HazeState?,shape: Shape): Modifier =
    if(sheetDocks()) clip(shape).background(MaterialTheme.colorScheme.surface) else drawerFrost(hazeState,shape)

/** Draws the docked sheets over the terminal side. The newest is on top; Back closes it. */
@Composable internal fun BoxScope.SheetDockHost(dock: SheetDock,modifier: Modifier=Modifier) {
    if(!dock.active || dock.entries.isEmpty()) return
    BackHandler {dock.entries.lastOrNull()?.onClose?.value?.invoke()}
    CompositionLocalProvider(LocalContentColor provides MaterialTheme.colorScheme.onSurface) {
        dock.entries.forEach {entry->key(entry) {
            // Cards rise into place; panels settle in over the side they cover.
            val enter=remember {Animatable(0f)}
            LaunchedEffect(Unit) {enter.animateTo(1f,spring(dampingRatio=.86f,stiffness=380f))}
            val rise=with(LocalDensity.current) {(if(entry.card) 48.dp else 14.dp).toPx()}
            val arrive=Modifier.graphicsLayer {alpha=enter.value.coerceIn(0f,1f);translationY=(1f-enter.value)*rise}
            if(entry.card) Box(modifier.matchParentSize().clip(WideCardShape).drawBehind {drawRect(Color.Black.copy(alpha=.45f*enter.value.coerceIn(0f,1f)))}
                .clickable(interactionSource=remember {MutableInteractionSource()},indication=null) {entry.onClose.value()}
                .semantics {contentDescription="Close sheet"},contentAlignment=Alignment.BottomCenter) {
                Box(Modifier.widthIn(max=520.dp).then(arrive).pointerInput(Unit) {detectTapGestures {}}) {entry.content.value()}
            } else Box(modifier.matchParentSize().then(arrive).clip(WideCardShape).background(MaterialTheme.colorScheme.surface)
                .border(1.dp,MaterialTheme.colorScheme.outline.copy(alpha=.18f),WideCardShape).pointerInput(Unit) {detectTapGestures {}}) {entry.content.value()}
        }}
    }
}

/** A docked card has no glass behind it, so it gets an edge to stand off the terminal. */
@Composable internal fun Modifier.dockedCardEdge(): Modifier =
    if(sheetDocks()) border(1.dp,MaterialTheme.colorScheme.outline.copy(alpha=.3f),RoundedCornerShape(30.dp)) else this
