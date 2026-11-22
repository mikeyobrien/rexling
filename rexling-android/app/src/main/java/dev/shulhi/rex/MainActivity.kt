package dev.shulhi.rex

import dev.chrisbanes.haze.HazeState
import dev.chrisbanes.haze.hazeSource

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.Manifest
import android.os.Build
import android.provider.Settings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.result.PickVisualMediaRequest
import androidx.compose.ui.platform.LocalContext
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.BackHandler
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.viewModels
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInRoot
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.hapticfeedback.HapticFeedbackType
import androidx.compose.ui.platform.LocalHapticFeedback
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.zIndex
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.view.WindowCompat

class MainActivity : ComponentActivity() {
    private val model: RexModel by viewModels()
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        consumeAlert(intent)
        enableEdgeToEdge(statusBarStyle=SystemBarStyle.dark(android.graphics.Color.TRANSPARENT),navigationBarStyle=SystemBarStyle.dark(android.graphics.Color.TRANSPARENT))
        setContent {
            val dark=when(model.appearance) {Appearance.SYSTEM->isSystemInDarkTheme();Appearance.DARK->true;Appearance.LIGHT->false}
            SideEffect {WindowCompat.getInsetsController(window,window.decorView).apply {
                isAppearanceLightStatusBars=!dark;isAppearanceLightNavigationBars=!dark
            }}
            RexTheme(dark,model.theme(dark)) {
                val dock=remember {SheetDock()}
                CompositionLocalProvider(LocalSheetDock provides dock) {RexApp(model)}
            }
        }
    }
    private fun consumeAlert(intent: Intent?) {
        AlertDestinations(BackgroundAlerts.preferences(this)).consume(intent?.data)?.let(model::openAlert)
        intent?.data=null
    }
    override fun onNewIntent(intent: Intent) {super.onNewIntent(intent);consumeAlert(intent)}
    override fun onWindowFocusChanged(hasFocus: Boolean) {super.onWindowFocusChanged(hasFocus);model.windowFocusChanged(hasFocus)}
    override fun onStart(){super.onStart();BackgroundAlerts.foreground.value=true;model.resume()}
    override fun onStop(){if(!isChangingConfigurations) BackgroundAlerts.foreground.value=false;model.pause();super.onStop()}
}

@Composable internal fun RexTheme(dark: Boolean,theme: AppTheme,content: @Composable ()->Unit) {
    CompositionLocalProvider(LocalTerminalPalette provides theme.terminal) {
        MaterialTheme(colorScheme=rexColorScheme(dark,theme.ui),content=content)
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable private fun RexApp(model: RexModel) {
    val client=model.activeClient
    val noticeHaze=remember {HazeState()}
    val captureNotice=rememberBackdropCapture(PortForwards.linkNotice!=null)
    Box(Modifier.fillMaxSize()) {
        CompositionLocalProvider(LocalKeepScreenAwake provides model.keepScreenAwake) {
            Box(Modifier.fillMaxSize().then(if(captureNotice) Modifier.hazeSource(noticeHaze) else Modifier)) {
                key(model.activeServerId) {RexWorkspace(model,client)}
            }
        }
        ForwardLinkUi(Modifier.align(Alignment.BottomEnd).windowInsetsPadding(WindowInsets.safeDrawing).imePadding().padding(end=12.dp,bottom=116.dp),hazeState=noticeHaze)
    }
    if(model.notificationNotice.isNotBlank()) KitDialog(onDismissRequest={model.notificationNotice=""},title={Text("Notification")},text={Text(model.notificationNotice)},confirmButton={TextButton(onClick={model.notificationNotice=""}) {Text("OK")}})
}

private enum class CommandStep { PALETTE, SPLIT, RENAME }
private data class CommandFlow(val target: TerminalCommandTarget,val restoreIme: Boolean,val step: CommandStep=CommandStep.PALETTE)

@OptIn(ExperimentalMaterial3Api::class)
@Composable private fun RexWorkspace(model: RexModel,client: RexConnection) {
    val state by client.chromeState.collectAsState()
    val navigation=client.navigationRequest
    val displayedSession=navigation.session ?: state.selectedSession
    val currentSessionKey=sessionKey(model.activeServerId,displayedSession)
    // With one server, naming it on every session line is noise; it shows once a second server exists.
    val sessionStrip=SessionCatalog.strip(model.sessionTargets,SessionTarget(model.activeServer,
        SessionItem(displayedSession,state.sessions.firstOrNull {it.id==displayedSession}?.label ?: if(state.connected) "No session" else state.phase)))
        .let {items->if(model.servers.size>1) items else items.map {it.copy(label=it.label.removeSuffix(" · ${model.activeServer.label}"))}}
    val displayedBlock=navigation.block ?: state.selectedBlock
    val changingSession=navigation.session!=null && navigation.session!=state.selectedSession
    val windows=if(changingSession) emptyList() else client.orderedWindows(state)
    val window=windows.firstOrNull {w->w.blocks.any {it.id==displayedBlock}} ?: windows.firstOrNull {it.blocks.isNotEmpty()}
    val activityBadges=remember(model.activitySources,model.activeServer,displayedSession,windows) {
        ActivityBadgePolicy.route(model.activitySources,model.activeServer,displayedSession,windows)
    }
    val ready=state.connected && !navigation.pending && !state.creating
    val keyTarget=client.keyTarget()
    val terminalHaze=remember {HazeState()}
    val workspaceHaze=remember {HazeState()}
    var readyBackdrop by remember {mutableStateOf(false)}
    var floatingBackdrop by remember {mutableStateOf(false)}
    var wheelBackdrop by remember {mutableStateOf(false)}
    val palette=LocalTerminalPalette.current
    var endpoint by rememberSaveable {mutableStateOf(client.savedEndpoint)}
    var showServer by rememberSaveable {mutableStateOf(true)}
    var servers by remember {mutableStateOf(false)}
    var addServer by remember {mutableStateOf(false)}
    var addEndpoint by remember {mutableStateOf("")}
    var forwards by remember {mutableStateOf(false)}
    var connectionError by remember {mutableStateOf<String?>(null)}
    var picker by remember {mutableStateOf("")}
    var newSession by remember {mutableStateOf(false)}
    var menu by remember {mutableStateOf(false)}
    var appearance by remember {mutableStateOf(false)}
    var about by remember {mutableStateOf(false)}
    var networkSettings by remember {mutableStateOf(false)}
    var sshKeysSettings by remember {mutableStateOf(false)}
    var preferencesOpen by remember {mutableStateOf(false)}
    var neovimSettings by remember {mutableStateOf(false)}
    var toolbarSettings by remember {mutableStateOf(false)}
    var controlSettings by remember {mutableStateOf(false)}
    var alerts by remember {mutableStateOf(false)}
    var commandFlow by remember {mutableStateOf<CommandFlow?>(null)}
    var restoreCommandIme by remember {mutableStateOf<TerminalCommandTarget?>(null)}
    val fontSize=model.fontSize
    var terminalView by remember {mutableStateOf<TerminalView?>(null)}
    val context=LocalContext.current
    var networkAction by remember {mutableStateOf<(() ->Unit)?>(null)}
    var networkDenied by remember {mutableStateOf<(() ->Unit)?>(null)}
    val localNetwork=rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {granted->
        val action=networkAction;val denied=networkDenied;networkAction=null;networkDenied=null
        if(granted) action?.invoke() else {denied?.invoke();model.images.notice="Allow local network access to connect to Rex."}
    }
    DisposableEffect(Unit) {onDispose {networkDenied?.invoke();networkAction=null;networkDenied=null}}
    fun withUploadNetwork(action: ()->Unit,onDenied: ()->Unit={}) {
        if(Build.VERSION.SDK_INT>=37 && context.checkSelfPermission("android.permission.ACCESS_LOCAL_NETWORK")!=android.content.pm.PackageManager.PERMISSION_GRANTED) {
            if(networkAction!=null) {onDenied();return}
            networkAction=action;networkDenied=onDenied;localNetwork.launch("android.permission.ACCESS_LOCAL_NETWORK")
        } else action()
    }
    val imageCommit: (android.net.Uri,()->Unit)->Boolean={uri,release->
        val target=model.imageTarget()
        if(target==null) model.attachImage(uri,target,release)
        else {withUploadNetwork({model.attachImage(uri,target,release)},release);true}
    }
    fun pasteClipboard(): Boolean = runCatching {
        pasteTerminalClipboard(context,{uri->imageCommit(uri,{})},client::paste)
    }.getOrElse {model.images.notice="Could not read the clipboard file. Try Attach file instead.";false}
    val photoPicker=rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) {uri->
        val target=model.pendingImageTarget;model.pendingImageTarget=null
        if(uri!=null) withUploadNetwork({model.attachImage(uri,target)})
    }
    fun chooseImage() {
        val target=model.imageTarget()
        if(target==null) {model.images.notice="Take control of a pane before attaching a file.";return}
        model.pendingImageTarget=target
        photoPicker.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly))
    }
    // Rex accepts any file type, so the document picker allows everything.
    val filePicker=rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) {uri->
        val target=model.pendingImageTarget;model.pendingImageTarget=null
        if(uri!=null) withUploadNetwork({model.attachImage(uri,target)})
    }
    fun chooseFile() {
        val target=model.imageTarget()
        if(target==null) {model.images.notice="Take control of a pane before attaching a file.";return}
        model.pendingImageTarget=target
        filePicker.launch(arrayOf("*/*"))
    }

    DisposableEffect(client) {onDispose {model.setPaneObscured(true)}}
    val keyboardDensity=LocalDensity.current
    val keyboardInsets=WindowInsets.ime
    val keyboardVisible by remember(keyboardInsets,keyboardDensity) {derivedStateOf {keyboardInsets.getBottom(keyboardDensity)>0}}
    // Follow where the keyboard is heading, not its animated inset: the inset stays above zero until
    // the hide animation ends, which left the toolbar showing after the keyboard was gone.
    @OptIn(ExperimentalLayoutApi::class) val keyboardShown=WindowInsets.isImeVisible
    val toolbarHidden=model.toolbarWithKeyboardOnly && !keyboardShown
    // The auto-hidden toolbar's unclipped height (divider + key row until measured). Written during its
    // measurement and read by the terminal's, so it is a plain holder rather than snapshot state.
    val toolbarHeight=remember(keyboardDensity) {intArrayOf(with(keyboardDensity) {1.dp.roundToPx()+45.dp.roundToPx()})}
    val blocks=window?.blocks ?: if(state.phase=="preview") listOf(BlockItem("preview","Terminal")) else emptyList()
    val chosen=displayedBlock.ifBlank {blocks.firstOrNull()?.id.orEmpty()}
    val floating=if(model.minimizeFloatingPanes) window?.floatingPanes().orEmpty() else emptyList()
    val carouselBlocks=window?.carouselPanes(model.minimizeFloatingPanes,chosen) ?: blocks
    // Same enabling as the former tab bar: tabs and panes switch only once navigation settles.
    val tabsReady=state.connected && !state.creating && !changingSession
    val chromeVisible=state.connected || state.phase=="preview" || client.hasScreen
    val tabEntries=windows.mapIndexed {index,w->
        val pane=client.tabPaneIndex(w)
        val block=w.blocks.getOrNull(pane)
        val program=state.programs[block?.id] ?: ProgramIdentity.Loading
        val floating=w.layout.filter {it.floating}.mapTo(hashSetOf()) {it.id}
        TabEntry(w.id,program.tabTitle(w.label,block?.label.orEmpty(),index+1,state.titles[block?.id].orEmpty()),
            listOfNotNull(if(w.blocks.size>1) "${w.blocks.size} panes" else null,state.directories[block?.id]?.takeIf {it.isNotBlank()}).joinToString(" · "),
            client.tabPrograms(w),pane,(activityBadges.tabs[w.id] ?: 0)>0,w.blocks.any {it.id in state.workingBlocks},w.label,
            w.blocks.map {b->
                val running=state.programs[b.id] ?: ProgramIdentity.Loading
                PaneEntry(b.id,running.paneTitle(b.label,state.titles[b.id].orEmpty()),state.directories[b.id].orEmpty(),b.label,running,b.id in floating,b.id in state.workingBlocks)
            },w.layout)
    }
    val tabHold=remember {TabHoldState()}
    var listHidden by rememberSaveable {mutableStateOf(false)}
    var headerTop by remember {mutableFloatStateOf(0f)}
    var headerHeight by remember {mutableIntStateOf(0)}
    val latestTabs by rememberUpdatedState(tabEntries)
    val titleHold=remember(client) {object: TitleHold {
        override fun start(y: Float)=tabHold.start(y)
        override fun move(y: Float)=tabHold.move(y)
        override fun end() {
            val index=tabHold.index(latestTabs.size)
            tabHold.finish()
            latestTabs.getOrNull(index)?.let {client.selectWindow(it.id)}
        }
    }}
    val floatingActive=floating.any {it.id==chosen}
    var hiddenFloating by rememberSaveable(displayedSession,window?.id) {mutableStateOf("")}
    val controlled=state.controlled && ready && client.hasScreen && (!floatingActive || hiddenFloating!=chosen)
    fun currentCommandTarget() = TerminalCommandTarget.capture(client.state.value,client.navigationRequest.pending)
    val commandsAvailable=TerminalCommandTarget.capture(state,navigation.pending)!=null && (!floatingActive || hiddenFloating!=chosen)
    fun openCommands() {
        if(!commandsAvailable || commandFlow!=null) return
        val target=currentCommandTarget() ?: return
        if(target.pane!=chosen || target.session!=displayedSession) return
        commandFlow=CommandFlow(target,controlled && keyboardVisible)
        terminalView?.hideKeyboard()
    }
    fun closeCommands(restore: Boolean=true) {
        val flow=commandFlow ?: return
        commandFlow=null
        restoreCommandIme=flow.target.takeIf {restore && flow.restoreIme}
    }
    fun copyViewport() {
        client.terminal.value?.let {screen->
            (context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(ClipData.newPlainText("Rex terminal",screen.plainText()))
        }
    }
    val activeCommands=commandFlow?.takeIf {it.target.matches(state,navigation.pending)}
    LaunchedEffect(commandFlow,activeCommands) {if(commandFlow!=null && activeCommands==null) closeCommands(false)}
    LaunchedEffect(restoreCommandIme,terminalView) {
        val target=restoreCommandIme ?: return@LaunchedEffect
        val view=terminalView ?: return@LaunchedEffect
        withFrameNanos { }
        restoreCommandIme=null
        if(target==currentCommandTarget() && controlled && view.isAttachedToWindow) view.keyboard()
    }
    SideEffect {model.setPaneObscured((floatingActive && hiddenFloating==chosen) || model.activityVisible || picker.isNotEmpty() || preferencesOpen || networkSettings || servers || addServer || forwards || newSession || appearance || about || neovimSettings || toolbarSettings || controlSettings || sshKeysSettings || alerts || commandFlow!=null)}
    fun openPane(id: String) {hiddenFloating="";client.selectBlock(id)}
    fun minimizeFloating() {
        hiddenFloating=chosen
        window?.let {client.floatingNavigation.returnPane(displayedSession,it)}?.let(client::selectBlock)
    }
    LaunchedEffect(navigation.id) {if(navigation.pending) hiddenFloating=""}
    LaunchedEffect(model.minimizeFloatingPanes) {if(model.minimizeFloatingPanes && floatingActive) minimizeFloating()}
    SideEffect {window?.let {client.floatingNavigation.remember(displayedSession,it,chosen)}}
    BackHandler(enabled=ready && model.minimizeFloatingPanes && !newSession && !appearance && !about && !neovimSettings && !alerts && floatingActive && hiddenFloating!=chosen && picker.isEmpty() && !preferencesOpen && !model.activityVisible) {minimizeFloating()}
    val liveBlock=state.selectedBlock.ifBlank {if(state.phase=="preview") "preview" else ""}
    // The header's second line states what is wrong, with the one thing to do about it.
    val paused=connectionStatus(state.phase,model.networkAvailable).takeIf {blocks.isNotEmpty()}
    // Settle first: every pane switch is briefly unowned before this device claims it.
    var viewing by remember {mutableStateOf(false)}
    LaunchedEffect(ready,client.hasScreen,state.controlled,chosen) {
        viewing=false
        if(ready && client.hasScreen && !state.controlled) {kotlinx.coroutines.delay(600);viewing=true}
    }
    val headerStatus=when {
        paused!=null->HeaderStatus(paused,StatusTone.WARNING,"Retry",model.networkAvailable!=false,client::retryNow)
        viewing->HeaderStatus("Viewing only",StatusTone.QUIET,"Take over",onAction=client::takeControl)
        else->null
    }
    // A tick under the finger for every selection, and a slide the way the tab change travels.
    val haptics=LocalHapticFeedback.current
    fun tick()=haptics.performHapticFeedback(HapticFeedbackType.SegmentTick)
    val travel=remember {TabTravel()}
    val tabIndex=windows.indexOfFirst {it.id==window?.id}
    LaunchedEffect(currentSessionKey,window?.id) {travel.arrive(currentSessionKey,window?.id.orEmpty(),tabIndex)}
    // One model for the switcher card and the wide list: what they show, and what each row can do.
    val sessionActivity=remember(model.activitySources) {ActivityBadgePolicy.sessions(model.activitySources)}
    val switcher=SwitcherModel(model.activeServer.label,
        sessionStrip.firstOrNull {it.id==currentSessionKey}?.label.orEmpty().removeSuffix(" · ${model.activeServer.label}"),
        tabEntries,window?.id.orEmpty(),chosen,
        model.sessionTargets.map {target->
            val activity=sessionActivity[target.key];val here=target.key==currentSessionKey
            SessionEntry(target,activity?.attention==true || (here && tabEntries.any {it.attention}),activity?.working==true || (here && tabEntries.any {it.working}))
        },currentSessionKey,
        enabled=tabsReady,ready=ready,canCreate=ready && state.selectedSession.isNotBlank(),canSplit=ready && state.selectedSession.isNotBlank() && chosen.isNotBlank(),
        busySession=model.closingSessionKey.ifBlank {model.renamingSessionKey},error=state.error,sessionError=model.sessionActionError,
        openTab={picker="";tick();client.selectWindow(it)},openPane={picker="";tick();openPane(it)},newTab={picker="";client.createTab()},
        split={anchor,split->picker="";client.createPane(split,anchor)},
        closeTab={tab->client.closeTab(state.selectedSession,tab.id,tab.panes.map {it.id})},closePane={pane->client.closePane(state.selectedSession,pane.id)},
        renameTab={tab,label->client.editTerminal(displayedSession,tab,tab,"rename_tab",label=label)},
        renamePane={tab,id,label->client.editTerminal(displayedSession,tab,id,"rename_pane",label=label)},
        moveTab=client::moveTab,movePane=client::movePane,
        floatPane={tab,id->client.editTerminal(displayedSession,tab,id,"float_pane")},
        dockPane={tab,id,anchor,split->client.editTerminal(displayedSession,tab,id,"dock_pane",anchor=anchor,split=split)},
        openSession={picker="";tick();model.selectSessionTarget(it)},newSession={picker="";newSession=true},refreshSessions={client.refresh();model.refreshSessionDetails()},
        closeSession=model::closeSession,renameSession=model::renameSession)
    LaunchedEffect(palette) { model.updateTerminalTheme(palette) }
    LaunchedEffect(state.phase) {if(state.connected || state.phase=="preview") showServer=false}
    LaunchedEffect(state.connected,state.selectedSession,state.selectedBlock,state.windows,navigation.pending) {
        client.openPreferredTerminal();client.rememberCurrentTerminal()
    }
    LaunchedEffect(state.connected,state.selectedSession,window?.id,state.selectedBlock,carouselBlocks) {
        val blocks=carouselBlocks
        val index=blocks.indexOfFirst {it.id==state.selectedBlock}
        if(index>=0 && blocks.size>1) client.prefetchPanes(listOf(blocks[Math.floorMod(index-1,blocks.size)].id,blocks[(index+1)%blocks.size].id).distinct())
    }
    LaunchedEffect(ready,chosen,client.hasScreen,hiddenFloating) {if(ready && (!floatingActive || hiddenFloating!=chosen)) client.claimActivePane()}
    LaunchedEffect(controlled,client.keyboardRequest,terminalView,commandFlow!=null) {
        if(controlled && commandFlow==null) {
            terminalView?.requestFocus()
            client.applySize()
            if(client.keyboardRequest==displayedBlock && terminalView!=null) {terminalView?.keyboard();client.keyboardShown()}
        }
    }
    val workspaceBackdrop=rememberBackdropCapture(commandFlow!=null || menu || forwards || preferencesOpen || servers || networkSettings || appearance || about || neovimSettings || toolbarSettings || controlSettings || sshKeysSettings || alerts || picker.isNotEmpty() || model.activityVisible)
    Surface(Modifier.fillMaxSize().then(if(workspaceBackdrop) Modifier.hazeSource(workspaceHaze) else Modifier),color=MaterialTheme.colorScheme.background) {
        // Wide screens (an unfolded Fold, tablets): the tab list sits beside the terminal; maximize hides it.
        BoxWithConstraints(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.safeDrawing).imePadding()) {
        val wide=maxWidth>=WideLayoutMinWidth
        // Sheets dock over the terminal side while there is a list beside it to keep in reach.
        val sheetDock=LocalSheetDock.current
        SideEffect {sheetDock?.active=wide && chromeVisible}
        Row(Modifier.fillMaxSize()) {
        if(wide && !listHidden && chromeVisible) {
            // The list names every session, so it needs their tab counts without opening the switcher.
            LaunchedEffect(state.connected,state.sessions.size) {model.refreshSessionDetails()}
            TabListPanel(WideListWidth,switcher,onSessions={picker="sessions"},onSettings={preferencesOpen=true},onCollapse={listHidden=true})
        } else if(wide && chromeVisible) TabRail(switcher) {listHidden=false}
        // Wide: the terminal side is a card like the list, with its header inside, both starting at the same height.
        Box(Modifier.weight(1f).fillMaxHeight()) {
        Column(Modifier.fillMaxSize().then(if(wide && chromeVisible) Modifier.padding(start=4.dp,end=8.dp,bottom=8.dp)
            .clip(WideCardShape).background(MaterialTheme.colorScheme.surface) else Modifier)) {
            // The header floats: the terminal runs beneath it, and scrolled-off lines fade out behind it.
            Box(Modifier.zIndex(1f).onGloballyPositioned {headerTop=it.positionInRoot().y;headerHeight=it.size.height}) {
            SessionHeader(sessionStrip,currentSessionKey,state.connected,model.sessionTargets.isNotEmpty(),
                blocks.map {state.programs[it.id] ?: ProgramIdentity.Loading},blocks.indexOfFirst {it.id==chosen}.coerceAtLeast(0),state.directories[displayedBlock].orEmpty(),{picker="sessions"},model::selectSessionTarget,model::jumpLastSession,
                serverIcons=if(model.servers.size>1) model.sessionTargets.associate {it.key to it.server.icon}+(currentSessionKey to model.activeServer.icon) else emptyMap(),
                working=chosen in state.workingBlocks,title=if(model.servers.isEmpty()) stringResource(R.string.app_name) else (state.programs[chosen] ?: ProgramIdentity.Loading).paneTitle(blocks.firstOrNull {it.id==chosen}?.label.orEmpty(),state.titles[chosen].orEmpty()),
                onTitleTap=if(tabsReady && windows.isNotEmpty()) {{picker="tabs"}} else null,
                onTitleHold=if(tabsReady && windows.size>1) titleHold else null,
                // Wide: the list shows tabs, so the header names the pane; phones keep the tab position.
                titleDetail=if(wide && chromeVisible) ""
                    else if(windows.size>1) "${windows.indexOfFirst {it.id==window?.id}+1}/${windows.size}" else "",
                status=headerStatus,titleAlpha={.25f+.75f*travel.progress.value},
                subtitle=if(wide && chromeVisible) state.directories[displayedBlock].orEmpty() else null,
                onTitleSwipe=if(tabsReady && carouselBlocks.size>1) {{step->
                    val index=carouselBlocks.indexOfFirst {it.id==chosen}.coerceAtLeast(0)
                    openPane(carouselBlocks[Math.floorMod(index+step,carouselBlocks.size)].id)
                }} else null,menu={
                    Row(verticalAlignment=Alignment.CenterVertically) {
                    if(chromeVisible) TerminalActions(state.connected && state.selectedSession.isNotBlank(),model.connectedCount>0,
                        {model.showActivity(true)},null,
                        // Everything that needs you and is not on screen: other sessions, and this session's other tabs.
                        attentionCount=activityBadges.elsewhere+activityBadges.tabs.filterKeys {it!=window?.id}.values.sum(),quiet=wide,
                        working=state.workingBlocks.any {it!=chosen} || sessionActivity.any {it.key!=currentSessionKey && it.value.working})
                    ChromeButton(ChromeIcon.MORE,"Terminal menu"){menu=true}
                    }
                })
            }
            if(tabHold.open) TabHoldList(tabHold,tabEntries,window?.id.orEmpty(),headerHeight,headerTop+headerHeight,hazeState=workspaceHaze)
            Box(Modifier.weight(1f).fillMaxWidth()) {
                Column(Modifier.fillMaxSize()) {
                    if(showServer || (state.phase=="disconnected"&&!client.hasScreen)) {
                        Surface(Modifier.padding(12.dp),shape=RoundedCornerShape(18.dp),color=MaterialTheme.colorScheme.surface) {
                            if(model.servers.isEmpty()) EmptyServerContent(onConnect={url->model.saveServer(null,"",url)},onMore={url->addEndpoint=url;addServer=true}) else Column(Modifier.padding(15.dp),verticalArrangement=Arrangement.spacedBy(8.dp)) {
                                Text(stringResource(R.string.app_name),fontWeight=FontWeight.Bold,fontSize=20.sp)
                                Text(stringResource(R.string.app_subtitle),fontSize=12.sp,color=MaterialTheme.colorScheme.onSurfaceVariant)
                                Text("Connect over Tailscale",fontWeight=FontWeight.SemiBold)
                                OutlinedTextField(endpoint,{endpoint=it},Modifier.fillMaxWidth(),label={Text("Rex server URL")},singleLine=true,
                                    keyboardOptions=KeyboardOptions(keyboardType=KeyboardType.Uri),shape=RoundedCornerShape(12.dp))
                                connectionError?.let {Text(it,color=MaterialTheme.colorScheme.error,fontSize=11.sp)}
                                Row(horizontalArrangement=Arrangement.spacedBy(8.dp)) {
                                    Button(onClick={connectionError=model.connect(endpoint)},enabled=state.phase!="connecting" && endpoint.isNotBlank()){Text("Connect")}
                                    TextButton(onClick={servers=true}) {Text("Servers")}
                                    if(state.connected || state.phase=="reconnecting") TextButton(onClick={model.disconnectServer(model.activeServerId)}){Text("Disconnect")}
                                    else TextButton(onClick={model.previewServer(model.activeServerId)}){Text("Try preview")}
                                    if(client.hasScreen) TextButton(onClick={showServer=false}){Text("Hide")}
                                }
                            }
                        }
                    }
                    if(blocks.isEmpty() && model.servers.isNotEmpty()) ConnectionNotice(state.phase,model.networkAvailable,client::retryNow)
                    if(blocks.isEmpty() && state.connected && !state.creating && state.sessions.isEmpty()) {
                        EmptySessionContent(model.activeServer.label,ready,{newSession=true},Modifier.weight(1f).fillMaxWidth())
                    }
                    // A session that ended on the server leaves no pane: name it rather than show a blank screen.
                    if(blocks.isEmpty() && state.connected && !state.creating && !navigation.pending && state.sessions.isNotEmpty()) {
                        EndedSessionContent(model.activeServer.label,ready,{picker="sessions"},{newSession=true},Modifier.weight(1f).fillMaxWidth())
                    }
                    if(state.error.isNotBlank() && state.phase !in listOf("connecting","reconnecting")) Text(state.error,Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.errorContainer).padding(10.dp),
                        color=MaterialTheme.colorScheme.onErrorContainer,fontSize=11.sp,maxLines=3,overflow=TextOverflow.Ellipsis)
                    if(blocks.isNotEmpty()) {
                        // Anything shown between the header and the terminal keeps the terminal below it.
                        val headerOverlap=if(showServer || (state.phase=="disconnected" && !client.hasScreen) ||
                            (state.error.isNotBlank() && state.phase !in listOf("connecting","reconnecting"))) 0 else headerHeight
                        Box(Modifier.weight(1f).fillMaxWidth().underHeader(headerOverlap).padding(bottom=3.dp).travelling(travel)) {
                        if(!floatingActive || hiddenFloating!=chosen) LiveTerminal(client,carouselBlocks,chosen,liveBlock,state.programs,controlled,fontSize.toFloat(),
                            client::type,client::scroll,client::wheel,client::takeControl,client::scrollToBottom,client::resize,::openPane,{picker="tabs"},{view->terminalView=view;view.onOpenLink={url->
                                val loopback=LoopbackLink.parse(url)
                                if(loopback==null) PortForwards.openBrowser(context,url)
                                else if(ready && chosen==liveBlock && displayedSession.isNotBlank()) {
                                    val sourceServer=model.activeServer;val sourceSession=displayedSession
                                    withUploadNetwork({if(model.servers.any {it.id==sourceServer.id && it.endpoint==sourceServer.endpoint}) PortForwards.openLink(context,sourceServer,sourceSession,loopback)})
                                }
                                else PortForwards.notice="Wait for this pane to connect before forwarding its link."
                            }},
                            Modifier.fillMaxSize(),topOverlapPx=headerOverlap,onNewPane={client.createPane()},newPaneEnabled=ready && state.selectedSession.isNotBlank() && chosen.isNotBlank(),floatingDock=floating.isNotEmpty(),paneLayout=window?.layout.orEmpty(),titles=state.titles,directories=state.directories,hazeState=terminalHaze,captureBackdrop=floatingBackdrop || wheelBackdrop || readyBackdrop || model.images.state!=null,onImage=imageCommit,onPaste=::pasteClipboard,imeEnterSubmits=model.enterSubmits,tapOpensKeyboard=model.toolbarWithKeyboardOnly,
                            onShortcut={shortcut-> when(shortcut) {
                                TerminalShortcut.PreviousPane -> {client.swipePane(-1);true}
                                TerminalShortcut.NextPane -> {client.swipePane(1);true}
                                TerminalShortcut.PreviousSession -> {client.swipeSession(-1);true}
                                TerminalShortcut.NextSession -> {client.swipeSession(1);true}
                                TerminalShortcut.NewTab -> {client.createTab();true}
                                TerminalShortcut.PanePicker -> {picker="panes";true}
                                TerminalShortcut.SessionPicker -> {picker="sessions";true}
                                TerminalShortcut.Keyboard -> {client.requestKeyboard();true}
                            }})
                        if(floatingActive && hiddenFloating==chosen) Box(Modifier.fillMaxSize(),contentAlignment=Alignment.Center) {
                            Text("Floating panes minimized",style=SettingsType.detail,color=MaterialTheme.colorScheme.onSurfaceVariant)
                        }

                        // Paused input shows as a dimmed terminal; the header says why, so nothing covers the text.
                        if(paused!=null) Box(Modifier.matchParentSize().background(MaterialTheme.colorScheme.background.copy(alpha=.5f)))
                        ReadyNotice(state.connectionEpoch,controlled,ready,Modifier.align(Alignment.TopCenter).padding(top=with(LocalDensity.current) {headerOverlap.toDp()}).padding(8.dp),hazeState=terminalHaze,onVisibleChange={readyBackdrop=it})
                        if((!floatingActive || hiddenFloating!=chosen) && model.neovimShortcutsEnabled && ready && chosen==liveBlock && chosen !in state.cachedPrograms && state.programs[chosen]?.kind==ProgramKind.NEOVIM && model.neovimShortcuts.isNotEmpty()) {
                            NeovimShortcutButton(model.neovimShortcuts,keyTarget,controlled,
                                onChoose={item,target->client.neovimShortcut(item,target)},onCustomize={neovimSettings=true},layout=model.neovimShortcutLayout,effect=model.neovimShortcutEffect,hapticsEnabled=model.neovimShortcutHaptics,hazeState=terminalHaze,onExpandedChange={wheelBackdrop=it},
                                modifier=Modifier.fillMaxSize(),position=model.neovimShortcutPosition,onPosition=model::moveNeovimShortcut,bottomInset=if(floating.isNotEmpty()) 64.dp else 10.dp)
                        }
                        if(floating.isNotEmpty()) FloatingPaneDock(floating,chosen.takeIf {floatingActive && hiddenFloating!=chosen},state.programs,state.titles,ready,
                            ::openPane,::minimizeFloating,Modifier.align(Alignment.BottomEnd).padding(end=6.dp),directories=state.directories,hazeState=terminalHaze,onExpandedChange={floatingBackdrop=it})
                        }
                    } else Box(Modifier.weight(1f).fillMaxWidth().padding(24.dp),contentAlignment=Alignment.Center) {
                        if(state.connected) Text("Choose a session, or start a new one.",fontFamily=MonoUi,fontSize=13.sp,color=MaterialTheme.colorScheme.onSurfaceVariant)
                    }
                    // Tabs and Activity live in the header; the key toolbar appears with the keyboard.
                    if(chromeVisible && keyboardVisible) {
                        // The bar's corner is the key's corner plus the gap around it, so the two curves stay parallel.
                        Column(Modifier.fillMaxWidth().riseWithKeyboard().clip(RoundedCornerShape(topStart=KeyRowCorner,topEnd=KeyRowCorner)).background(MaterialTheme.colorScheme.surface)) {
                            KeyboardBar(controlled,ready,client.controlModifier,client.altModifier,client.shiftModifier,keyboardVisible,
                                client::toggleControl,client::toggleAlt,client::toggleShift,client::pressShift,client::releaseShift,{client.keyForTarget(it,keyTarget)},readable=state.connected,onControlShortcut={client.controlShortcut(it,keyTarget)},inputKey=keyTarget,shortcutGesture=model.shortcutGesture,arrowJoystickEnabled=model.arrowJoystickEnabled,onPaste={pasteClipboard()},onPrompt=if(client.hasSemanticPrompts && !changingSession) client::scrollPrompt else null,toolbarKeys=model.toolbarKeys,controlItems=model.controlItems,onCommands=if(commandsAvailable) ::openCommands else null) {
                                if(!controlled) client.takeControl() else if(keyboardVisible) terminalView?.hideKeyboard() else terminalView?.keyboard()
                            }
                        }
                    }
                }
                model.images.state?.let {status->ImageUploadOverlay(status,model.images::retry,model.images::cancel,
                    modifier=Modifier.align(Alignment.BottomEnd).padding(horizontal=16.dp).padding(bottom=(if(state.connected && keyboardVisible) 66.dp else 16.dp)+(if(state.connected && floating.isNotEmpty()) 40.dp else 0.dp)),retryable=model.images.retryable,hazeState=terminalHaze)}
                // Loading feedback must not resize the terminal grid during navigation.
                if(state.creating || navigation.pending || state.phase=="connecting" || state.phase=="reconnecting")
                    LinearProgressIndicator(Modifier.align(Alignment.TopCenter).fillMaxWidth().height(2.dp))

            }
        }
        sheetDock?.let {SheetDockHost(it,Modifier.padding(start=4.dp,end=8.dp,bottom=8.dp))}
        }
        }
        }
    }
    // Sessions, tabs and panes share one card: the title opens it on the tabs, "sessions" one level up.
    if(menu) ActionCard(ActionCardModel(
        overline=listOf(switcher.session,switcher.server).filter {it.isNotBlank()}.joinToString(" · "),
        title=(state.programs[chosen] ?: ProgramIdentity.Loading).paneTitle(blocks.firstOrNull {it.id==chosen}?.label.orEmpty(),state.titles[chosen].orEmpty()),
        status=paused ?: if(!state.connected) state.phase.replaceFirstChar(Char::uppercaseChar) else if(controlled) "In control" else "Viewing only",
        controlled=controlled && paused==null,size=client.screenDimensions?.let {"${it.first} × ${it.second}"}.orEmpty(),
        ownership=if(client.hasScreen) CardAction(if(controlled) "Release" else "Take over",ChromeIcon.POINTER,
            run=if(ready) ({if(controlled) client.releaseControl() else client.takeControl()}) else null) else null,
        tiles=listOf(
            CardAction("Paste",ChromeIcon.PASTE,run=if(controlled) ({pasteClipboard()}) else null),
            CardAction("Image",ChromeIcon.IMAGE,run=if(controlled && model.images.state==null) ::chooseImage else null),
            CardAction("Copy",ChromeIcon.COPY,run=if(client.hasScreen) ({
                client.terminal.value?.let {screen->(getClipboard(model.getApplication()) as ClipboardManager).setPrimaryClip(ClipData.newPlainText("Rex terminal",screen.plainText()))}
            }) else null),
            // The key row's keyboard button only shows with the keyboard; mouse-mode apps take taps.
            CardAction("Keyboard",ChromeIcon.KEYBOARD,run=if(ready && client.hasScreen) client::requestKeyboard else null)),
        places=listOf(
            CardAction("New session",ChromeIcon.ADD,run=if(ready) ({newSession=true}) else null),
            CardAction("Commands",ChromeIcon.COMMANDS,run=if(commandsAvailable) ::openCommands else null),
            CardAction("Attach file",ChromeIcon.FILE,run=if(controlled && model.images.state==null) ::chooseFile else null),
            CardAction("Port forwarding",ChromeIcon.SERVER,run={forwards=true}),
            // One server that is connected needs no count.
            CardAction("Servers",ChromeIcon.SERVER,detail=if(model.servers.size>1 || model.connectedCount==0) "${model.connectedCount} connected" else "",run={servers=true})),
        preferences={preferencesOpen=true},
        // The pane in front of you; a tab's only pane takes the tab with it, so it says so.
        close=RowManager(RowsState(),switcher).close(if(blocks.size>1) paneKey(chosen) else tabKey(window?.id.orEmpty()))
            ?.let {CardClose(if(blocks.size>1) "Close pane" else "Close tab",it.question,it.detail,if(ready) it.run else null)}),onClose={menu=false},hazeState=workspaceHaze)
    if(picker.isNotEmpty()) Switcher(switcher,onClose={picker=""},sessionsFirst=picker=="sessions",hazeState=workspaceHaze)
    if(model.activityVisible) ActivitySheet(model.activitySources,model::refreshActivity,model::openActivity,onClose={model.showActivity(false)},onSeen=model::markActivitySeen,onMarkAll=model::markAllActivitySeen,hazeState=workspaceHaze)
    NetworkPrompts()
    if(sshKeysSettings) SshKeysSheet(onClose={sshKeysSettings=false},hazeState=workspaceHaze)
    if(networkSettings) NetworkSheet(model.activeServer,state.connected,hazeState=workspaceHaze) {networkSettings=false}
    if(preferencesOpen) PreferencesSheet(onClose={preferencesOpen=false},
        appearance=model.appearance,onAppearance=model::changeAppearance,onColors={preferencesOpen=false;appearance=true},
        fontSize=fontSize,onFontSize=model::changeFontSize,keepScreenAwake=model.keepScreenAwake,onKeepScreenAwake=model::changeKeepScreenAwake,
        arrowJoystickEnabled=model.arrowJoystickEnabled,onArrowJoystick=model::changeArrowJoystick,shortcutGesture=model.shortcutGesture,onShortcutGesture=model::changeShortcutGesture,
        toolbarKeys=model.toolbarKeys,onToolbar={preferencesOpen=false;toolbarSettings=true},onNeovim={preferencesOpen=false;neovimSettings=true},
        minimizeFloatingPanes=model.minimizeFloatingPanes,onMinimizeFloatingPanes=model::changeMinimizeFloatingPanes,
        onNetwork={preferencesOpen=false;networkSettings=true},onAlerts={preferencesOpen=false;alerts=true},onForwards={preferencesOpen=false;forwards=true},
        onControlItems={preferencesOpen=false;controlSettings=true},onSshKeys={preferencesOpen=false;sshKeysSettings=true},
        codexTitleAttention=model.experimentalCodexAttention,onCodexTitleAttention=model::changeCodexAttention,onAbout={preferencesOpen=false;about=true},hazeState=workspaceHaze)
    if(controlSettings) ControlShortcutsSheet(model.controlItems,model::saveControlShortcuts,onClose={controlSettings=false},hazeState=workspaceHaze)
    if(toolbarSettings) KeyboardToolbarSheet(model.toolbarKeys,model::changeToolbar,onClose={toolbarSettings=false},hazeState=workspaceHaze)
    if(neovimSettings) NeovimShortcutsSheet(model.neovimShortcuts,model::saveNeovimShortcuts,onClose={neovimSettings=false},
        enabled=model.neovimShortcutsEnabled,onEnabled=model::enableNeovimShortcuts,
        layout=model.neovimShortcutLayout,onLayout=model::changeNeovimShortcutLayout,effect=model.neovimShortcutEffect,onEffect=model::changeNeovimShortcutEffect,
        hapticsEnabled=model.neovimShortcutHaptics,onHaptics=model::changeNeovimShortcutHaptics,hazeState=workspaceHaze)
    if(model.images.notice.isNotEmpty()) KitDialog(onDismissRequest={model.images.notice=""},title={Text("File upload")},text={Text(model.images.notice)},confirmButton={TextButton(onClick={model.images.notice=""}) {Text("OK")}})
    if(about) AboutSheet(hazeState=workspaceHaze) {about=false}
    if(alerts) BackgroundAlertsDialog(model,hazeState=workspaceHaze) {alerts=false}
    activeCommands?.let {flow->
        val tab=windows.firstOrNull {it.id==flow.target.tab}
        val block=tab?.blocks?.firstOrNull {it.id==flow.target.pane}
        when(flow.step) {
            CommandStep.PALETTE->TerminalCommandsSheet(
                (state.programs[flow.target.pane] ?: ProgramIdentity.Loading).paneTitle(block?.label.orEmpty(),state.titles[flow.target.pane].orEmpty()),
                canCopy=client.hasScreen,catalog=state.serverCommands,serverRun=state.serverCommandRun,
                loadServerCommands={client.loadServerCommands(flow.target)},
                runServerCommand={name,id->client.runServerCommand(flow.target,name,id)},onClose={closeCommands()},hazeState=workspaceHaze,onChoose={command->
                    if(currentCommandTarget()!=flow.target) closeCommands(false)
                    else when(command) {
                        TerminalCommand.SPLIT->commandFlow=flow.copy(step=CommandStep.SPLIT)
                        TerminalCommand.RENAME->commandFlow=flow.copy(step=CommandStep.RENAME)
                        TerminalCommand.COPY->{if(client.hasScreen) copyViewport();closeCommands()}
                        TerminalCommand.FORWARDS->{closeCommands(false);forwards=true}
                    }
                })
            CommandStep.SPLIT->CommandSplitDialog(onClose={closeCommands()},onChoose={split->
                if(currentCommandTarget()==flow.target) client.createPane(split,flow.target.pane)
                closeCommands(false)
            })
            CommandStep.RENAME->TerminalRenameDialog(TerminalRenameTarget(flow.target.tab,flow.target.tab,true,tab?.label.orEmpty()),
                enabled=ready,onClose={closeCommands()},onSave={label->
                    currentCommandTarget()==flow.target && client.editTerminal(flow.target.session,flow.target.tab,flow.target.tab,"rename_tab",label=label)
                })
        }
    }
    if(forwards) PortForwardSheet(model.forwardServers(),model.activeServerId,workspaceHaze,onClose={forwards=false},onStart={id->
        val saved=PortForwards.items.firstOrNull {it.id==id}
        val target=model.forwardServers().firstOrNull {it.profile.id==saved?.serverId && it.profile.endpoint==saved.endpoint}
        if(saved!=null && target!=null && target.session.isNotBlank()) PortForwards.update(saved.copy(session=target.session,server=target.profile.label))
        withUploadNetwork({PortForwards.requestStart(context,id)})
    }) {target,host,port,local->
        withUploadNetwork({PortForwards.start(context,target.profile,target.session,host,port,local)})
    }
    if(servers) ServersSheet(model,hazeState=workspaceHaze){servers=false}
    if(addServer) ServerEditor(null,onClose={addServer=false},initialEndpoint=addEndpoint,onSave={label,url,icon->model.saveServer(null,label,url,icon)})
    if(appearance) AppearanceSheet(model.appearance,model.themes,model.theme(false),model.theme(true),model::changeAppearance,model::chooseTheme,hazeState=workspaceHaze) {appearance=false}
    if(newSession) NewSessionDialog(model.activeServer.label,ready,client::createSession) {newSession=false}

}
private fun getClipboard(context: Context)=context.getSystemService(Context.CLIPBOARD_SERVICE)

@Composable private fun LiveTerminal(client: RexConnection,blocks: List<BlockItem>,chosen: String,liveBlock: String,
    programs: Map<String,ProgramIdentity>,controlled: Boolean,fontSize: Float,
    onInput: (String)->Unit,onScroll: (Int)->Unit,onWheel: (Int,Int,Int,Int,Int)->Unit,onActivate: ()->Unit,onBottom: ()->Unit,onDimensions: (Int,Int)->Unit,
    onSelect: (String)->Unit,onPanes: ()->Unit,onView: (TerminalView)->Unit,modifier: Modifier=Modifier,
    onNewPane: (()->Unit)?=null,newPaneEnabled: Boolean=true,floatingDock: Boolean=false,paneLayout: List<PanePlacement> = emptyList(),titles: Map<String,String> = emptyMap(),directories: Map<String,String> = emptyMap(),hazeState: HazeState?=null,captureBackdrop: Boolean=false,onImage: (android.net.Uri,()->Unit)->Boolean={_,release->release();false},onPaste: ()->Boolean={false},onShortcut: (TerminalShortcut)->Boolean={false},imeEnterSubmits: Boolean=false,tapOpensKeyboard: Boolean=false,topOverlapPx: Int=0) {
    val layout by client.displayLayout.collectAsState()
    val screen=client.screenForLayout(layout)
    TerminalCarousel(blocks,chosen,liveBlock,screen,client.paneFrames,programs,controlled,fontSize,
        onInput,onScroll,onWheel,onActivate,onBottom,onDimensions,onSelect,onPanes,onView,modifier,client::bindTerminalView,onMouseClick=client::click,mouseTarget=client::keyTarget,onImage=onImage,onPaste=onPaste,imageTarget=client::imageInputTarget,bottomControlsInset=if(floatingDock) 56.dp else 0.dp,paneLayout=paneLayout,titles=titles,directories=directories,hazeState=hazeState,captureBackdrop=captureBackdrop,onNewPane=onNewPane,newPaneEnabled=newPaneEnabled,onShortcut=onShortcut,imeEnterSubmits=imeEnterSubmits,tapOpensKeyboard=tapOpensKeyboard,topOverlapPx=topOverlapPx)
}

@Composable private fun BackgroundAlertsDialog(model: RexModel,hazeState: HazeState?=null,onClose: ()->Unit) {
    val context=LocalContext.current
    var denied by remember {mutableStateOf(false)}
    val permission=rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) {granted->
        if(granted) {model.setBackgroundAlerts(true);denied=false} else denied=true
    }
    SettingsDrawer("Background alerts","Close background alerts",onClose,hazeState) {
        Column(Modifier.fillMaxWidth().heightIn(max=sheetBodyMax(600.dp)).verticalScroll(rememberScrollState()).padding(horizontal=16.dp).padding(top=8.dp,bottom=12.dp),verticalArrangement=Arrangement.spacedBy(12.dp)) {
            Text("Get notified when a task finishes, needs input, approval, sign-in, or fails while Rexling is in the background or your phone is locked.",style=SettingsType.body)
            Text("A quiet monitoring notification stays visible. Stop it there, or turn this off. Closing Rexling stops monitoring.",style=SettingsType.body)
            Text("Your phone may delay alerts while sleeping.",style=SettingsType.detail,color=MaterialTheme.colorScheme.onSurfaceVariant)
            Row(Modifier.fillMaxWidth(),verticalAlignment=Alignment.CenterVertically) {
                Text("Notify about task updates",Modifier.weight(1f),style=SettingsType.row)
                Switch(checked=model.backgroundAlerts,onCheckedChange={enabled->
                    if(!enabled) model.setBackgroundAlerts(false)
                    else if(Build.VERSION.SDK_INT>=33 && context.checkSelfPermission(Manifest.permission.POST_NOTIFICATIONS)!=android.content.pm.PackageManager.PERMISSION_GRANTED) permission.launch(Manifest.permission.POST_NOTIFICATIONS)
                    else if(BackgroundAlerts.allowed(context)) {model.setBackgroundAlerts(true);denied=false} else denied=true
                },enabled=model.servers.any {model.serverStatuses[it.id]?.maintained==true})
            }
            if(model.servers.none {model.serverStatuses[it.id]?.maintained==true}) Text("Connect a server to enable alerts.",style=SettingsType.detail)
            if(denied || (model.backgroundAlerts && !BackgroundAlerts.allowed(context))) {
                Text("Allow notifications in Android settings to receive alerts.",style=SettingsType.detail)
                TextButton(onClick={context.startActivity(Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE,context.packageName))}) {Text("Notification settings")}
            }
        }
            TextButton(onClick=onClose,modifier=Modifier.align(Alignment.End)) {Text("Done")}
    }
}
