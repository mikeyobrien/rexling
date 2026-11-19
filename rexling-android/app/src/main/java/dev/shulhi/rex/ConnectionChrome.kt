package dev.shulhi.rex

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.*
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.delay

internal fun connectionMessage(phase: String,network: Boolean?): String? = when {
    phase !in listOf("connecting","reconnecting","error")->null
    network==false->"Waiting for network · input paused"
    phase=="connecting"->"Connecting…"
    else->"Reconnecting · input paused"
}
@Composable internal fun ConnectionNotice(phase: String,network: Boolean?,onRetry: ()->Unit,modifier: Modifier=Modifier) {
    connectionMessage(phase,network)?.let {message->
        Row(modifier.fillMaxWidth().clip(RoundedCornerShape(14.dp)).background(MaterialTheme.colorScheme.surfaceVariant).padding(start=12.dp,end=6.dp),
            verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(8.dp)) {
            Text(message,Modifier.weight(1f),fontSize=11.sp,color=MaterialTheme.colorScheme.onSurfaceVariant)
            TextButton(onClick=onRetry,enabled=network!=false,contentPadding=PaddingValues(horizontal=8.dp,vertical=0.dp)) {Text("Retry now",fontSize=11.sp)}
        }
    }
}
/** What the header's second line says instead of the session, and the one thing to do about it. */
internal enum class StatusTone {WARNING,QUIET}
internal class HeaderStatus(val text: String,val tone: StatusTone,val action: String,val enabled: Boolean=true,val onAction: ()->Unit)

/** The connection as the header states it. Input is paused whenever this is not null. */
internal fun connectionStatus(phase: String,network: Boolean?): String? = when {
    phase !in listOf("connecting","reconnecting","error")->null
    network==false->"Offline"
    phase=="connecting"->"Connecting…"
    else->"Reconnecting…"
}

@Composable internal fun OwnershipMenuItem(controlled: Boolean,enabled: Boolean,onClick: ()->Unit) {
    TerminalMenuItem(if(controlled) "Release control" else "Take control",ChromeIcon.POINTER,enabled=enabled,onClick=onClick)
}
@Composable internal fun ReadyNotice(epoch: Long,controlled: Boolean,ready: Boolean,modifier: Modifier=Modifier,hazeState: dev.chrisbanes.haze.HazeState?=null,onVisibleChange: (Boolean)->Unit={}) {
    var shown by remember {mutableLongStateOf(0)};var visible by remember {mutableStateOf(false)}
    LaunchedEffect(epoch,controlled,ready) {
        if(epoch>shown && controlled && ready) {shown=epoch;visible=true;try {delay(1400)} finally {visible=false}}
    }
    val notify by rememberUpdatedState(onVisibleChange)
    val localHaze=remember {dev.chrisbanes.haze.HazeState()}
    LaunchedEffect(visible) {notify(visible)}
    DisposableEffect(Unit) {onDispose {notify(false)}}
    if(visible) Text("Ready to type",modifier.paneFrost(hazeState ?: localHaze,RoundedCornerShape(12.dp),tint=MaterialTheme.colorScheme.primaryContainer,edge=false).padding(horizontal=12.dp,vertical=5.dp),
        fontSize=10.sp,color=MaterialTheme.colorScheme.onPrimaryContainer)
}
