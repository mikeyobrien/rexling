package dev.shulhi.rex

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import dev.chrisbanes.haze.HazeState

@OptIn(ExperimentalMaterial3Api::class)
@Composable internal fun TerminalCommandsSheet(paneTitle: String,canCopy: Boolean,
    catalog: ServerCommandCatalog?,serverRun: ServerCommandRun?,loadServerCommands: ()->Boolean,
    runServerCommand: (String,Long)->Boolean,onChoose: (TerminalCommand)->Unit,
    onClose: ()->Unit,hazeState: HazeState?) {
    var query by remember {mutableStateOf("")}
    val scroll=rememberScrollState()
    val commands=remember(query,canCopy) {TerminalCommand.matching(query,canCopy)}
    val hostCommands=remember(query,catalog) {catalog?.actions.orEmpty().filter {it.matches(query)}}
    var loadFailed by remember {mutableStateOf(false)}
    var requestId by remember {mutableStateOf<Long?>(null)}
    var requestFailed by remember {mutableStateOf(false)}
    val status=serverRun?.takeIf {it.id==requestId}
    val running=requestId!=null && status?.running!=false && !requestFailed
    LaunchedEffect(Unit) {loadFailed=!loadServerCommands()}
    LaunchedEffect(query) {scroll.scrollTo(0)}
    ModalBottomSheet(onDismissRequest=onClose,sheetState=rememberModalBottomSheetState(skipPartiallyExpanded=true),
        containerColor=MaterialTheme.colorScheme.surfaceContainerHigh,contentColor=MaterialTheme.colorScheme.onSurface,
        dragHandle=null) {
        Column(Modifier.drawerFrost(hazeState,BottomSheetDefaults.ExpandedShape).fillMaxWidth().imePadding().padding(horizontal=8.dp).padding(bottom=8.dp)) {
            Row(Modifier.fillMaxWidth().heightIn(min=48.dp).padding(start=12.dp),verticalAlignment=Alignment.CenterVertically,
                horizontalArrangement=Arrangement.spacedBy(10.dp)) {
                Text("Commands",style=MaterialTheme.typography.titleMedium,fontWeight=FontWeight.Bold)
                Text(paneTitle,Modifier.weight(1f),style=SettingsType.detail,color=MaterialTheme.colorScheme.onSurfaceVariant,maxLines=1,overflow=TextOverflow.Ellipsis)
                ChromeButton(ChromeIcon.CLOSE,"Close commands",onClick=onClose)
            }
            Column(Modifier.fillMaxWidth().weight(1f,fill=false).verticalScroll(scroll)) {
                OutlinedTextField(query,{query=it},Modifier.fillMaxWidth().padding(horizontal=8.dp,vertical=4.dp),
                    label={Text("Search commands")},singleLine=true,shape=RoundedCornerShape(12.dp),
                    leadingIcon={ChromeSymbol(ChromeIcon.SEARCH,color=MaterialTheme.colorScheme.onSurfaceVariant)})
                if(requestId!=null) {
                    val error=if(requestFailed) "Command could not be sent" else status?.error.orEmpty()
                    val message=when {
                        running -> "Running command…"
                        error.isNotEmpty() -> error
                        status?.message?.isNotBlank()==true -> status.message
                        else -> "Command completed"
                    }
                    Text(message,Modifier.padding(14.dp),style=SettingsType.detail,
                        color=if(error.isNotEmpty()) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurface)
                    status?.warning?.takeIf {it.isNotBlank()}?.let {Text(it,Modifier.padding(horizontal=14.dp),style=SettingsType.detail)}
                }
                commands.forEach {command->
                    Row(Modifier.fillMaxWidth().heightIn(min=48.dp).clickable(role=Role.Button) {onChoose(command)}.padding(horizontal=14.dp,vertical=8.dp),
                        horizontalArrangement=Arrangement.spacedBy(12.dp),verticalAlignment=Alignment.CenterVertically) {
                        ChromeSymbol(command.icon,color=MaterialTheme.colorScheme.primary)
                        Text(command.label,Modifier.weight(1f),style=SettingsType.row)
                        Text(command.scope,style=SettingsType.detail,color=MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                }
                val catalogError=if(loadFailed) "Could not load host commands" else catalog?.error.orEmpty()
                if(query.isBlank() || hostCommands.isNotEmpty() || catalogError.isNotEmpty()) {
                    HorizontalDivider(Modifier.padding(horizontal=14.dp,vertical=4.dp))
                    Text("Host commands",Modifier.padding(start=14.dp,top=8.dp,bottom=4.dp),style=SettingsType.detail,
                        color=MaterialTheme.colorScheme.onSurfaceVariant)
                    when {
                        catalogError.isNotEmpty() -> {
                            Text(catalogError,Modifier.padding(horizontal=14.dp),style=SettingsType.detail,color=MaterialTheme.colorScheme.error)
                            TextButton(onClick={loadFailed=!loadServerCommands()}) {Text("Retry")}
                        }
                        catalog==null -> Text("Loading…",Modifier.padding(14.dp),style=SettingsType.detail)
                        catalog.actions.isEmpty() -> Text("No commands configured",Modifier.padding(14.dp),style=SettingsType.detail)
                    }
                    hostCommands.forEach {action->
                        Row(Modifier.fillMaxWidth().heightIn(min=48.dp)
                            .clickable(role=Role.Button,enabled=!running && serverRun?.running!=true && !action.requiresArguments) {
                                val id=android.os.SystemClock.elapsedRealtimeNanos()
                                requestId=id
                                requestFailed=!runServerCommand(action.name,id)
                            }.padding(horizontal=14.dp,vertical=8.dp),
                            horizontalArrangement=Arrangement.spacedBy(12.dp),verticalAlignment=Alignment.CenterVertically) {
                            ChromeSymbol(ChromeIcon.COMMANDS,color=MaterialTheme.colorScheme.primary)
                            Text(action.title,Modifier.weight(1f),style=SettingsType.row,
                                color=if(action.requiresArguments) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface)
                            if(action.requiresArguments) Text("Requires arguments",style=SettingsType.detail,color=MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
                if(query.isNotBlank() && commands.isEmpty() && hostCommands.isEmpty() && catalogError.isEmpty())
                    Text("No matching commands",Modifier.padding(14.dp),style=SettingsType.detail,color=MaterialTheme.colorScheme.onSurfaceVariant)
            }
        }
    }
}

@Composable internal fun CommandSplitDialog(onChoose: (PaneSplit)->Unit,onClose: ()->Unit) {
    AlertDialog(onDismissRequest=onClose,title={Text("Split pane")},text={
        Column {
            PaneSplit.entries.forEach {split->
                TextButton(onClick={onChoose(split)},modifier=Modifier.fillMaxWidth()) {
                    SplitDiagram(split,Modifier.size(24.dp),MaterialTheme.colorScheme.primary)
                    Spacer(Modifier.width(12.dp))
                    Text(split.label,Modifier.weight(1f))
                }
            }
        }
    },confirmButton={},dismissButton={TextButton(onClick=onClose) {Text("Cancel")}})
}
