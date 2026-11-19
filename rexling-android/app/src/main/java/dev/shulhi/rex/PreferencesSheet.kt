package dev.shulhi.rex

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
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

internal const val MinFontSize=8
internal const val MaxFontSize=20

/** Every setting shows its choices in its own row, so none of them needs a sentence underneath. */
@Composable internal fun PreferencesSheet(onClose: ()->Unit,
    appearance: Appearance=Appearance.SYSTEM,onAppearance: (Appearance)->Unit={},onColors: ()->Unit={},
    fontSize: Int=10,onFontSize: (Int)->Unit={},
    keepScreenAwake: KeepScreenAwake=KeepScreenAwake.SYSTEM,onKeepScreenAwake: (KeepScreenAwake)->Unit={},
    arrowJoystickEnabled: Boolean=true,onArrowJoystick: (Boolean)->Unit={},
    shortcutGesture: ControlShortcutGesture=ControlShortcutGesture.HOLD,onShortcutGesture: (ControlShortcutGesture)->Unit={},
    toolbarKeys: List<ToolbarKey> = ToolbarLayout.defaults,onToolbar: ()->Unit={},onNeovim: ()->Unit={},
    minimizeFloatingPanes: Boolean=false,onMinimizeFloatingPanes: (Boolean)->Unit={},
    onNetwork: ()->Unit={},onAlerts: ()->Unit={},onForwards: ()->Unit={},
    codexTitleAttention: Boolean=false,onCodexTitleAttention: (Boolean)->Unit={},onAbout: ()->Unit={},hazeState: dev.chrisbanes.haze.HazeState?=null) {
    SettingsDrawer("Preferences","Close preferences",onClose,hazeState) {
        Column(Modifier.fillMaxWidth().heightIn(max=sheetBodyMax(640.dp)).verticalScroll(rememberScrollState()).padding(horizontal=12.dp).padding(top=4.dp,bottom=12.dp),
            verticalArrangement=Arrangement.spacedBy(10.dp)) {
            ChoiceGroup {
                ChoiceRow("Theme") {Segmented(listOf(Appearance.SYSTEM to "System",Appearance.DARK to "Dark",Appearance.LIGHT to "Light"),appearance,onAppearance)}
                GoRow("Colors",onColors)
                ChoiceRow("Text size") {
                    Row(verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(4.dp)) {
                        StepButton("−","Decrease text size",fontSize>MinFontSize) {onFontSize(fontSize-1)}
                        Text("$fontSize",Modifier.widthIn(min=34.dp).semantics {contentDescription="Text size $fontSize"},fontFamily=MonoUi,fontSize=15.sp,textAlign=androidx.compose.ui.text.style.TextAlign.Center)
                        StepButton("+","Increase text size",fontSize<MaxFontSize) {onFontSize(fontSize+1)}
                    }
                }
                ChoiceRow("Keep awake",last=true) {Segmented(listOf(KeepScreenAwake.SYSTEM to "Off",KeepScreenAwake.VISIBLE to "Visible",KeepScreenAwake.ACTIVE to "Active"),keepScreenAwake,onKeepScreenAwake)}
            }
            ChoiceGroup {
                ChoiceRow("Arrows") {Segmented(listOf(true to "Joystick",false to "Keys"),arrowJoystickEnabled,onArrowJoystick)}
                ChoiceRow("Ctrl menu") {Segmented(listOf(ControlShortcutGesture.HOLD to "Hold",ControlShortcutGesture.DOUBLE_TAP to "Double tap"),shortcutGesture,onShortcutGesture)}
                // The row previews itself: these are the keys, in order.
                GoRow("Key row",onToolbar) {
                    Row(horizontalArrangement=Arrangement.spacedBy(3.dp)) {
                        ToolbarLayout.normalize(toolbarKeys).take(5).forEach {key->
                            Box(Modifier.height(22.dp).widthIn(min=26.dp).clip(RoundedCornerShape(4.dp)).background(MaterialTheme.colorScheme.surfaceVariant).padding(horizontal=4.dp),contentAlignment=Alignment.Center) {
                                if(key.icon!=null) ChromeSymbol(key.icon,Modifier.size(12.dp))
                                else Text(key.cap,fontFamily=MonoUi,fontSize=9.sp,maxLines=1,softWrap=false)
                            }
                        }
                    }
                }
                GoRow("Neovim keys",onNeovim)
                ChoiceRow("Floating panes",last=true) {Segmented(listOf(false to "Swipe",true to "Dock"),minimizeFloatingPanes,onMinimizeFloatingPanes)}
            }
            ChoiceGroup {
                GoRow("Network",onNetwork)
                GoRow("Alerts",onAlerts)
                GoRow("Port forwarding",onForwards)
                Row(Modifier.fillMaxWidth().heightIn(min=52.dp).toggleable(codexTitleAttention,role=Role.Switch,onValueChange=onCodexTitleAttention).padding(horizontal=16.dp),
                    verticalAlignment=Alignment.CenterVertically) {
                    Text("Codex title attention",Modifier.weight(1f),style=SettingsType.row,fontSize=15.sp)
                    Switch(checked=codexTitleAttention,onCheckedChange=null)
                }
                RowDivider()
                GoRow("About",onAbout,last=true)
            }
        }
    }
}

@Composable private fun ChoiceGroup(content: @Composable ColumnScope.()->Unit) {
    Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(22.dp)).background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha=.45f)),content=content)
}
@Composable private fun RowDivider() {
    HorizontalDivider(Modifier.padding(horizontal=16.dp),color=MaterialTheme.colorScheme.outline.copy(alpha=.22f))
}

/** A setting whose choices are the row. */
@Composable private fun ChoiceRow(label: String,last: Boolean=false,control: @Composable ()->Unit) {
    Row(Modifier.fillMaxWidth().heightIn(min=52.dp).padding(start=16.dp,end=10.dp),verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(8.dp)) {
        Text(label,Modifier.weight(1f),style=SettingsType.row,fontSize=15.sp,maxLines=1,overflow=TextOverflow.Ellipsis)
        control()
    }
    if(!last) RowDivider()
}

/** A row that goes somewhere, with an optional preview of what is there. */
@Composable private fun GoRow(label: String,onClick: ()->Unit,last: Boolean=false,preview: (@Composable ()->Unit)?=null) {
    Row(Modifier.fillMaxWidth().heightIn(min=52.dp).clickable(role=Role.Button,onClick=onClick).padding(horizontal=16.dp),
        verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(10.dp)) {
        Text(label,Modifier.weight(1f),style=SettingsType.row,fontSize=15.sp,maxLines=1,overflow=TextOverflow.Ellipsis)
        preview?.invoke()
        ChromeSymbol(ChromeIcon.CHEVRON,Modifier.size(14.dp).graphicsLayer {rotationZ=-90f},MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha=.8f))
    }
    if(!last) RowDivider()
}

/** A segmented choice: every option visible, the chosen one filled. */
@Composable internal fun <T> Segmented(options: List<Pair<T,String>>,selected: T,onSelect: (T)->Unit) {
    val scheme=MaterialTheme.colorScheme
    Row(Modifier.selectableGroup(),horizontalArrangement=Arrangement.spacedBy(2.dp)) {
        options.forEach {(value,label)->
            val active=value==selected
            Box(Modifier.heightIn(min=34.dp).clip(RoundedCornerShape(10.dp)).background(if(active) scheme.primary.copy(alpha=.32f) else Color.Transparent)
                .selectable(active,role=Role.RadioButton) {onSelect(value)}.padding(horizontal=12.dp),contentAlignment=Alignment.Center) {
                Text(label,fontSize=13.sp,fontWeight=if(active) FontWeight.SemiBold else FontWeight.Normal,color=if(active) scheme.onSurface else scheme.onSurfaceVariant,maxLines=1,softWrap=false)
            }
        }
    }
}

@Composable private fun StepButton(symbol: String,description: String,enabled: Boolean,onClick: ()->Unit) {
    Box(Modifier.size(34.dp).clip(CircleShape).background(MaterialTheme.colorScheme.surfaceVariant)
        .clickable(enabled=enabled,role=Role.Button,onClick=onClick).semantics {contentDescription=description},contentAlignment=Alignment.Center) {
        Text(symbol,fontSize=18.sp,color=MaterialTheme.colorScheme.onSurface.copy(alpha=if(enabled) 1f else .38f))
    }
}
