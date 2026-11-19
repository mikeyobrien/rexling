package dev.shulhi.rex

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import dev.chrisbanes.haze.HazeState

/**
 * The one card every sheet is: a title, a round close button and the content. On a phone it floats
 * above the bottom edge; docked on a wide screen it fills the terminal side.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable internal fun SheetCard(title: String,closeDescription: String,onClose: ()->Unit,hazeState: HazeState?=null,modifier: Modifier=Modifier,
    overline: String="",onBack: (() -> Unit)?=null,swipeToDismiss: Boolean=true,actions: @Composable RowScope.()->Unit={},content: @Composable ColumnScope.()->Unit) {
    val docked=sheetDocks()
    val localHaze=remember {HazeState()}
    // Built outside the sheet's window: inside, the frost's host would be that window and redraw it every frame.
    val surface=if(swipeToDismiss || docked) Modifier.sheetSurface(hazeState,if(docked) WideCardShape else Kit.card)
        else Modifier.paneFrost(hazeState ?: localHaze,Kit.card,edge=false,opacity=.72f,blur=24.dp)
    @Composable fun Body(outer: Modifier=Modifier) {
        Box(outer.fillMaxWidth().then(if(docked) Modifier else Modifier.windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom)).padding(horizontal=10.dp).padding(bottom=10.dp))) {
            Column(modifier.then(surface).fillMaxWidth().padding(vertical=12.dp)) {
                Row(Modifier.fillMaxWidth().heightIn(min=52.dp).padding(start=if(onBack!=null) 12.dp else 22.dp,end=12.dp),verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(8.dp)) {
                    onBack?.let {KitRoundButton(ChromeIcon.LEFT,"Back to appearance",onClick=it)}
                    Column(Modifier.weight(1f)) {
                        if(overline.isNotBlank()) Text(overline,style=Kit.meta,fontWeight=FontWeight.Medium,color=MaterialTheme.colorScheme.onSurfaceVariant,maxLines=1,overflow=TextOverflow.Ellipsis)
                        Text(title,style=Kit.title,maxLines=1,overflow=TextOverflow.Ellipsis)
                    }
                    actions()
                    KitRoundButton(ChromeIcon.CLOSE,closeDescription,onClick=onClose)
                }
                content()
            }
        }
    }
    if(!swipeToDismiss && !docked) {
        // Editors with drag-and-drop must not compete with a sheet's draggable or nested-scroll
        // dismiss handlers, so they get the same card in a fixed bottom dialog.
        Dialog(onDismissRequest=onClose,properties=DialogProperties(usePlatformDefaultWidth=false,decorFitsSystemWindows=false)) {
            Box(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Top))) {
                Box(Modifier.matchParentSize().clickable(interactionSource=remember {MutableInteractionSource()},indication=null,onClick=onClose))
                CompositionLocalProvider(LocalContentColor provides MaterialTheme.colorScheme.onSurface) {
                    Body(Modifier.align(Alignment.BottomCenter).widthIn(max=BottomSheetDefaults.SheetMaxWidth).pointerInput(Unit) {detectTapGestures(onTap={})})
                }
            }
        }
    } else SheetWindow(onClose) {Body()}
}

/** A settings sheet: the card with nothing beside its title. */
@Composable internal fun SettingsDrawer(title: String,closeDescription: String,onClose: ()->Unit,
    hazeState: HazeState?=null,onBack: (() -> Unit)?=null,swipeToDismiss: Boolean=true,content: @Composable ColumnScope.()->Unit) =
    SheetCard(title,closeDescription,onClose,hazeState,onBack=onBack,swipeToDismiss=swipeToDismiss,content=content)
