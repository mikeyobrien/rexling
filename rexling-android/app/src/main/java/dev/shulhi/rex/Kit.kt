package dev.shulhi.rex

import androidx.compose.foundation.background
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
import androidx.compose.ui.graphics.compositeOver
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.window.DialogProperties

/**
 * The one kit every screen is built from: four shapes, four type sizes, and four colours that each
 * mean one thing. A new sheet, row or dialog takes these rather than choosing its own.
 */
internal object Kit {
    /** A card: every sheet, menu and dialog. */
    val card=RoundedCornerShape(30.dp)
    /** A group of rows inside a card. */
    val group=RoundedCornerShape(22.dp)
    /** One row, tile or field. */
    val row=RoundedCornerShape(14.dp)
    /** A keycap. */
    val key=RoundedCornerShape(7.dp)

    val title=TextStyle(fontSize=20.sp,lineHeight=26.sp,fontWeight=FontWeight.Bold)
    val rowText=TextStyle(fontSize=15.sp,lineHeight=20.sp,fontWeight=FontWeight.Medium)
    val meta=TextStyle(fontSize=12.sp,lineHeight=17.sp)
    val label=TextStyle(fontSize=11.sp,lineHeight=15.sp,fontWeight=FontWeight.SemiBold,letterSpacing=1.sp)

    /** The fill of a group or a neutral control, on a card. */
    @Composable fun groupFill()=MaterialTheme.colorScheme.surfaceVariant.copy(alpha=.55f)
    /** Blue: selected, or working. */
    @Composable fun active()=MaterialTheme.colorScheme.primary
    /** Amber: needs you. */
    @Composable fun attention()=attentionColor()
    /** Green: done. */
    @Composable fun done()=if(LocalTerminalPalette.current.dark) Color(0xff5fc98a) else Color(0xff2f8f58)
    /** Red: destroys something. */
    @Composable fun destructive()=destructiveColor()
}

internal enum class KitTone {PRIMARY,NEUTRAL,DESTRUCTIVE}

/** The one button: filled blue for the thing to do, a neutral pill otherwise, red-tinted when it destroys. */
@Composable internal fun KitButton(label: String,onClick: ()->Unit,modifier: Modifier=Modifier,tone: KitTone=KitTone.NEUTRAL,enabled: Boolean=true,icon: ChromeIcon?=null) {
    val scheme=MaterialTheme.colorScheme
    val fill=when(tone) {KitTone.PRIMARY->scheme.primary;KitTone.NEUTRAL->Kit.groupFill();KitTone.DESTRUCTIVE->Kit.destructive().copy(alpha=.2f)}
    val ink=when(tone) {KitTone.PRIMARY->scheme.onPrimary;KitTone.NEUTRAL->scheme.onSurface;KitTone.DESTRUCTIVE->Kit.destructive()}.copy(alpha=if(enabled) 1f else .38f)
    Row(modifier.heightIn(min=48.dp).clip(CircleShape).background(if(enabled) fill else fill.copy(alpha=fill.alpha*.5f))
        .clickable(enabled=enabled,role=Role.Button,onClick=onClick).padding(horizontal=20.dp),
        verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.Center) {
        if(icon!=null) ChromeSymbol(icon,Modifier.padding(end=8.dp).size(18.dp),ink)
        Text(label,style=Kit.rowText,color=ink,maxLines=1,overflow=TextOverflow.Ellipsis)
    }
}

/** The round button in a card's corner: close, back, add. */
@Composable internal fun KitRoundButton(icon: ChromeIcon,description: String,modifier: Modifier=Modifier,enabled: Boolean=true,onClick: ()->Unit) {
    Box(modifier.size(44.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha=.6f))
        .clickable(enabled=enabled,role=Role.Button,onClick=onClick).semantics {contentDescription=description},contentAlignment=Alignment.Center) {
        ChromeSymbol(icon,Modifier.size(18.dp),MaterialTheme.colorScheme.onSurface.copy(alpha=if(enabled) 1f else .38f))
    }
}

/** A dialog is the same card, asking one thing. Takes what a Material dialog takes. */
@Composable internal fun KitDialog(onDismissRequest: ()->Unit,confirmButton: @Composable ()->Unit,modifier: Modifier=Modifier,dismissButton: (@Composable ()->Unit)?=null,
    title: (@Composable ()->Unit)?=null,text: (@Composable ()->Unit)?=null,properties: DialogProperties=DialogProperties()) {
    val scheme=MaterialTheme.colorScheme
    AlertDialog(onDismissRequest=onDismissRequest,confirmButton=confirmButton,modifier=modifier,dismissButton=dismissButton,
        title=title?.let {{ProvideTextStyle(Kit.title,it)}},text=text,shape=Kit.card,
        // The card's own colour, opaque: a dialog has no glass behind it.
        containerColor=scheme.surfaceVariant.copy(alpha=.35f).compositeOver(scheme.surface),tonalElevation=0.dp,
        titleContentColor=scheme.onSurface,textContentColor=scheme.onSurfaceVariant,properties=properties)
}
