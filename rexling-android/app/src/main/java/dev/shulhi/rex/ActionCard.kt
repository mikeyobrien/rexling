package dev.shulhi.rex

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.chrisbanes.haze.HazeState

/** One thing the action card can do. Null [run] shows it disabled. */
internal class CardAction(val label: String,val icon: ChromeIcon,val detail: String="",val run: (()->Unit)?)

/**
 * What the terminal menu shows: the pane's state as chips, the everyday actions as tiles and the
 * places you can go from here as rows.
 */
internal class ActionCardModel(val overline: String,val title: String,
    /** "In control", "Viewing only" or what the connection is doing. */
    val status: String,val controlled: Boolean,
    /** The grid as "63 × 58", blank until there is a screen. */
    val size: String,
    /** "Release" or "Take over". */
    val ownership: CardAction?,val tiles: List<CardAction>,val places: List<CardAction>,val preferences: ()->Unit)

private val CardGroupShape=Kit.group

@OptIn(ExperimentalMaterial3Api::class)
@Composable internal fun ActionCard(model: ActionCardModel,onClose: ()->Unit,hazeState: HazeState?=null) {
    val scheme=MaterialTheme.colorScheme
    val group=scheme.surfaceVariant.copy(alpha=.55f)
    // Every action leaves the card first, so what it opens is not stacked on top of it.
    fun leave(run: ()->Unit): ()->Unit={onClose();run()}
    // Built outside the sheet: inside, the frost's host would be the sheet's own window and redraw it every frame.
    val frost=Modifier.sheetSurface(hazeState,Kit.card)
    SheetWindow(onClose,card=true) {
        Column(Modifier.fillMaxWidth().windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom)).padding(horizontal=10.dp).padding(bottom=10.dp)
            .then(frost).dockedCardEdge().padding(12.dp).semantics {contentDescription="Terminal actions"},verticalArrangement=Arrangement.spacedBy(10.dp)) {
            Row(verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(8.dp)) {
                Column(Modifier.weight(1f).padding(start=10.dp)) {
                    if(model.overline.isNotBlank()) Text(model.overline,fontSize=12.sp,fontWeight=FontWeight.Medium,color=scheme.onSurfaceVariant,maxLines=1,overflow=TextOverflow.Ellipsis)
                    Text(model.title,fontSize=20.sp,fontWeight=FontWeight.Bold,maxLines=1,overflow=TextOverflow.Ellipsis)
                }
                KitRoundButton(ChromeIcon.SETTINGS,"Preferences",onClick=leave(model.preferences))
                KitRoundButton(ChromeIcon.CLOSE,"Close actions",onClick=onClose)
            }
            Row(Modifier.fillMaxWidth().heightIn(min=36.dp),verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(8.dp)) {
                val tint=if(model.controlled) scheme.primary else scheme.onSurfaceVariant
                Row(Modifier.clip(CircleShape).background(if(model.controlled) scheme.primary.copy(alpha=.18f) else group).padding(horizontal=12.dp,vertical=7.dp)
                    .semantics(mergeDescendants=true) {contentDescription="Status"},verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(6.dp)) {
                    if(model.controlled) ChromeSymbol(ChromeIcon.POINTER,Modifier.size(15.dp),tint)
                    Text(model.status,fontSize=14.sp,fontWeight=FontWeight.Medium,color=tint,maxLines=1)
                }
                if(model.size.isNotBlank()) Text(model.size,Modifier.clip(CircleShape).border(1.dp,scheme.outline.copy(alpha=.5f),CircleShape).padding(horizontal=12.dp,vertical=7.dp)
                    .semantics {contentDescription="Terminal size ${model.size}"},fontFamily=MonoUi,fontSize=13.sp,maxLines=1)
                Spacer(Modifier.weight(1f))
                model.ownership?.let {action->
                    Text(action.label,Modifier.clip(CircleShape).clickable(enabled=action.run!=null,role=Role.Button,onClick=leave(action.run ?: {})).padding(horizontal=10.dp,vertical=7.dp),
                        fontSize=15.sp,fontWeight=FontWeight.SemiBold,color=scheme.primary.copy(alpha=if(action.run!=null) 1f else .38f))
                }
            }
            Row(Modifier.fillMaxWidth(),horizontalArrangement=Arrangement.spacedBy(8.dp)) {
                model.tiles.forEach {action->
                    val color=scheme.onSurface.copy(alpha=if(action.run!=null) 1f else .38f)
                    Column(Modifier.weight(1f).height(76.dp).clip(RoundedCornerShape(20.dp)).background(group)
                        .clickable(enabled=action.run!=null,role=Role.Button,onClick=leave(action.run ?: {})).semantics(mergeDescendants=true) {},
                        horizontalAlignment=Alignment.CenterHorizontally,verticalArrangement=Arrangement.spacedBy(8.dp,Alignment.CenterVertically)) {
                        ChromeSymbol(action.icon,Modifier.size(22.dp),color)
                        Text(action.label,fontSize=13.sp,color=color,maxLines=1)
                    }
                }
            }
            Column(Modifier.fillMaxWidth().clip(CardGroupShape).background(group)) {
                model.places.forEachIndexed {index,action->
                    if(index>0) HorizontalDivider(Modifier.padding(start=56.dp,end=16.dp),color=scheme.outline.copy(alpha=.25f))
                    val color=scheme.onSurface.copy(alpha=if(action.run!=null) 1f else .38f)
                    Row(Modifier.fillMaxWidth().heightIn(min=56.dp).clickable(enabled=action.run!=null,role=Role.Button,onClick=leave(action.run ?: {}))
                        .padding(horizontal=18.dp).semantics(mergeDescendants=true) {},verticalAlignment=Alignment.CenterVertically) {
                        ChromeSymbol(action.icon,Modifier.size(20.dp),scheme.onSurfaceVariant.copy(alpha=if(action.run!=null) 1f else .38f))
                        Column(Modifier.weight(1f).padding(start=18.dp)) {
                            Text(action.label,fontSize=16.sp,fontWeight=FontWeight.Medium,color=color,maxLines=1,overflow=TextOverflow.Ellipsis)
                            if(action.detail.isNotBlank()) Text(action.detail,fontSize=12.sp,color=scheme.onSurfaceVariant,maxLines=1,overflow=TextOverflow.Ellipsis)
                        }
                        ChromeSymbol(ChromeIcon.CHEVRON,Modifier.size(16.dp).graphicsLayer {rotationZ=-90f},scheme.onSurfaceVariant.copy(alpha=.7f))
                    }
                }
            }
        }
    }
}
