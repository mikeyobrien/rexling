package dev.shulhi.rex

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import kotlinx.coroutines.launch

/** First run: the address is the only thing needed. The name comes from the host; the rest waits behind [onMore]. */
@Composable internal fun EmptyServerContent(onConnect: (String)->String?,onMore: (String)->Unit) {
    val scheme=MaterialTheme.colorScheme
    var address by rememberSaveable {mutableStateOf("")}
    var error by remember {mutableStateOf<String?>(null)}
    fun connect() {if(address.isNotBlank()) error=onConnect(ServerProfile.completeAddress(address))}
    Column(Modifier.fillMaxWidth().padding(16.dp),verticalArrangement=Arrangement.spacedBy(12.dp)) {
        Text("Connect to Rex",Modifier.padding(start=4.dp,top=4.dp),style=SettingsType.title)
        BasicTextField(address,{address=it;error=null},Modifier.fillMaxWidth().height(52.dp).clip(RoundedCornerShape(16.dp)).border(1.dp,scheme.primary.copy(alpha=.6f),RoundedCornerShape(16.dp))
            .padding(horizontal=16.dp).semantics {contentDescription="Server address"},
            textStyle=TextStyle(fontFamily=MonoUi,fontSize=15.sp,color=scheme.onSurface),singleLine=true,cursorBrush=SolidColor(scheme.primary),
            keyboardOptions=KeyboardOptions(keyboardType=KeyboardType.Uri,imeAction=ImeAction.Go,autoCorrectEnabled=false),keyboardActions=KeyboardActions(onGo={connect()}),
            decorationBox={field->Row(Modifier.fillMaxHeight(),verticalAlignment=Alignment.CenterVertically,horizontalArrangement=Arrangement.spacedBy(12.dp)) {
                ServerGlyph(ServerIcon.GLOBE,Modifier.size(18.dp))
                Box(Modifier.weight(1f)) {if(address.isEmpty()) Text("rex.example.ts.net",fontFamily=MonoUi,fontSize=15.sp,color=scheme.onSurfaceVariant.copy(alpha=.6f));field()}
            }})
        error?.let {Text(it,Modifier.padding(horizontal=4.dp),style=SettingsType.detail,color=scheme.error)}
        Button(onClick=::connect,enabled=address.isNotBlank(),modifier=Modifier.fillMaxWidth().height(50.dp)) {Text("Connect",fontSize=15.sp)}
        Row(Modifier.fillMaxWidth().heightIn(min=44.dp).clip(RoundedCornerShape(12.dp)).clickable(role=androidx.compose.ui.semantics.Role.Button) {onMore(ServerProfile.completeAddress(address))}.padding(horizontal=6.dp),
            verticalAlignment=Alignment.CenterVertically) {
            Text("Name, icon, network",Modifier.weight(1f),style=SettingsType.body,color=scheme.onSurfaceVariant)
            ChromeSymbol(ChromeIcon.CHEVRON,Modifier.size(16.dp),scheme.onSurfaceVariant)
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable internal fun ServersSheet(model: RexModel,hazeState: dev.chrisbanes.haze.HazeState?=null,onDismiss: ()->Unit) {
    var editing by remember {mutableStateOf<ServerProfile?>(null)}
    var editor by remember {mutableStateOf(false)}
    SheetWindow(onDismiss) {
        Column(Modifier.sheetSurface(hazeState,BottomSheetDefaults.ExpandedShape).fillMaxWidth().windowInsetsPadding(WindowInsets.safeDrawing.only(WindowInsetsSides.Bottom))) {
            DrawerHandle()
            Row(Modifier.fillMaxWidth().heightIn(min=48.dp).padding(start=16.dp,end=4.dp),verticalAlignment=Alignment.CenterVertically) {
                Text("Servers",Modifier.weight(1f),style=SettingsType.row,fontWeight=FontWeight.Bold)
                ChromeButton(ChromeIcon.ADD,"Add server") {editing=null;editor=true}
                ChromeButton(ChromeIcon.CLOSE,"Close servers",onClick=onDismiss)
            }
        Column(Modifier.fillMaxWidth().heightIn(max=sheetBodyMax(560.dp)).verticalScroll(rememberScrollState()).padding(horizontal=16.dp).padding(bottom=12.dp)) {
            Text("${model.connectedCount} connected",Modifier.padding(top=4.dp,bottom=8.dp),style=SettingsType.detail,color=MaterialTheme.colorScheme.onSurfaceVariant)
            if(model.servers.isEmpty()) Text("Add a Rex server to get started.",Modifier.padding(vertical=16.dp))
            for(server in model.servers) {
                val selected=server.id==model.activeServerId
                val status=model.serverStatuses[server.id] ?: ServerStatus()
                var menu by remember(server.id) {mutableStateOf(false)}
                Row(Modifier.fillMaxWidth().padding(bottom=4.dp).heightIn(min=56.dp).clip(RoundedCornerShape(12.dp))
                    .background(if(selected) MaterialTheme.colorScheme.primaryContainer.copy(alpha=.65f) else MaterialTheme.colorScheme.surfaceVariant.copy(alpha=.45f))
                    .clickable {onDismiss();model.connectServer(server.id)}.padding(start=12.dp,top=8.dp,bottom=8.dp),verticalAlignment=Alignment.CenterVertically) {
                    Box(Modifier.size(7.dp).background(if(status.connected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outline,RoundedCornerShape(4.dp)))
                    Spacer(Modifier.width(11.dp))
                    ServerGlyph(server.icon,Modifier.size(22.dp))
                    Spacer(Modifier.width(10.dp))
                    Column(Modifier.weight(1f)) {
                        Text(server.label,style=SettingsType.row,maxLines=1,overflow=TextOverflow.Ellipsis)
                        Text(server.endpoint,style=SettingsType.detail,color=MaterialTheme.colorScheme.onSurfaceVariant,maxLines=1,overflow=TextOverflow.Ellipsis)
                        Text(when(status.phase) {
                            "connected"->if(selected) "Connected · viewing" else "Connected"
                            "connecting"->"Connecting…";"reconnecting"->"Reconnecting…";"paused"->"Paused"
                            "error"->"Connection failed";else->"Disconnected"
                        },style=SettingsType.detail,color=if(status.connected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant)
                        if(status.error.isNotBlank()) Text(status.error,style=SettingsType.detail,maxLines=2,overflow=TextOverflow.Ellipsis,color=MaterialTheme.colorScheme.error)
                    }
                    Box {
                        ChromeButton(ChromeIcon.MORE,"Server actions for ${server.label}") {menu=true}
                        DropdownMenu(menu,{menu=false},containerColor=MaterialTheme.colorScheme.surface,tonalElevation=0.dp) {
                            TerminalMenuItem("Edit",ChromeIcon.EDIT,onClick={menu=false;editing=server;editor=true})
                            if(status.maintained) TerminalMenuItem("Disconnect",ChromeIcon.UNPLUG,onClick={menu=false;model.disconnectServer(server.id)})
                            TerminalMenuItem("Forget server",ChromeIcon.DELETE,onClick={menu=false;model.forgetServer(server.id)})
                        }
                    }
                }
            }
            Text("Switch servers without closing their connections.",Modifier.padding(top=9.dp),style=SettingsType.detail,color=MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
    }
    if(editor) ServerEditor(editing,onClose={editor=false},onSave={label,endpoint,icon->model.saveServer(editing?.id,label,endpoint,icon)})
}

@Composable internal fun ServerEditor(server: ServerProfile?,onClose: ()->Unit,onSave: (String,String,ServerIcon)->String?,initialEndpoint: String="") {
    val networkStatus by NetworkSettings.status.collectAsState()
    val tailnetConnected=networkStatus.optJSONObject("tailscale")?.optString("BackendState")=="Running"
    var name by remember(server?.id) {mutableStateOf(server?.label.orEmpty())}
    var endpoint by remember(server?.id) {mutableStateOf(server?.endpoint ?: initialEndpoint)}
    var icon by remember(server?.id) {mutableStateOf(server?.icon ?: ServerIcon.GLOBE)}
    var error by remember {mutableStateOf<String?>(null)}
    var route by remember(server?.id) {mutableStateOf(NetworkSettings.route(server?.endpoint ?: initialEndpoint))}
    var routesOpen by remember {mutableStateOf(false)}
    var networkOpen by remember {mutableStateOf(false)}
    var busy by remember {mutableStateOf(false)}
    val scope=rememberCoroutineScope()
    // Opened from a dialog, so it has to rise above it rather than dock beneath.
    if(networkOpen) CompositionLocalProvider(LocalSheetDock provides null) {NetworkSheet(onClose={networkOpen=false})}
    AlertDialog(onDismissRequest={if(!busy) onClose()},title={Text(if(server==null) "Add server" else "Edit server",style=SettingsType.title)},text={
        Column(Modifier.verticalScroll(rememberScrollState()),verticalArrangement=Arrangement.spacedBy(10.dp)) {
            Row(Modifier.fillMaxWidth(),horizontalArrangement=Arrangement.spacedBy(10.dp),verticalAlignment=Alignment.CenterVertically) {
                ServerIconPicker(icon,compact=true) {icon=it}
                OutlinedTextField(name,{name=it;error=null},Modifier.weight(1f),label={Text("Name")},singleLine=true,textStyle=SettingsType.body)
            }
            OutlinedTextField(endpoint,{endpoint=it;error=null},Modifier.fillMaxWidth(),label={Text("Server URL")},singleLine=true,textStyle=SettingsType.body,
                keyboardOptions=KeyboardOptions(keyboardType=KeyboardType.Uri),placeholder={Text("https://host.tailnet.ts.net",fontSize=11.sp)})
            Text(if(endpoint.startsWith("ssh://")) "SSH to a host running Rex." else "Use an HTTPS address, or ssh://user@host.",style=SettingsType.detail,color=MaterialTheme.colorScheme.onSurfaceVariant)
            if(runCatching {ServerProfile.normalizeEndpoint(endpoint)}.isSuccess) Column(Modifier.fillMaxWidth().clip(RoundedCornerShape(12.dp)).background(MaterialTheme.colorScheme.surfaceVariant.copy(alpha=.5f)).padding(12.dp)) {
                Text("Connection network",style=SettingsType.detail,color=MaterialTheme.colorScheme.onSurfaceVariant)
                Box {
                    TextButton(enabled=!busy,onClick={routesOpen=true},contentPadding=PaddingValues(horizontal=0.dp,vertical=4.dp)) {Text(route.label,style=SettingsType.row);Text(" ▾")}
                    DropdownMenu(expanded=routesOpen,onDismissRequest={routesOpen=false}) {
                        ServerNetworkRoute.entries.forEach {choice->DropdownMenuItem(text={Text(choice.label,style=SettingsType.row)},onClick={route=choice;routesOpen=false})}
                    }
                }
                if(route==ServerNetworkRoute.BUILTIN) {
                    Text("Use a tailnet IP or device name. No Tailscale VPN app required.",style=SettingsType.detail,color=MaterialTheme.colorScheme.onSurfaceVariant)
                    TextButton(onClick={networkOpen=true},contentPadding=PaddingValues(horizontal=0.dp,vertical=4.dp)) {Text(if(tailnetConnected) "Connected" else "Set up / sign in",style=SettingsType.detail)}
                } else Text("Use this phone’s network and existing VPN.",style=SettingsType.detail,color=MaterialTheme.colorScheme.onSurfaceVariant)
            }
            if(endpoint.startsWith("ssh://")) {
                val normalized=runCatching {ServerProfile.normalizeEndpoint(endpoint)}.getOrNull()
                if(normalized!=null) SshKeyEditor(normalized)
            }
            error?.let {Text(it,color=MaterialTheme.colorScheme.error,fontSize=12.sp)}
        }
    },confirmButton={Button(onClick={
        val profile=runCatching {ServerProfile.create(name,endpoint,icon=icon)}.getOrElse {error=it.message;return@Button}
        val requestedRoute=route
        busy=true
        scope.launch {
            val previous=NetworkSettings.route(profile.endpoint)
            error=runCatching {NetworkSettings.saveRoute(profile.endpoint,requestedRoute) ?: onSave(profile.label,profile.endpoint,profile.icon)}.getOrElse {"Could not save server network"}
            if(error!=null && previous!=requestedRoute) NetworkSettings.saveRoute(profile.endpoint,previous)
            busy=false;if(error==null) onClose()
        }
    },enabled=endpoint.isNotBlank() && !busy) {Text(if(busy) "Saving…" else "Save & connect")}},
        dismissButton={TextButton(enabled=!busy,onClick=onClose) {Text("Cancel")}})
}
