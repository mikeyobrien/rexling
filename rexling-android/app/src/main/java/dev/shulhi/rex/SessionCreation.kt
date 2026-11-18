package dev.shulhi.rex

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable internal fun NewSessionDialog(serverLabel: String,enabled: Boolean,onCreate: (String)->Boolean,onDismiss: ()->Unit) {
    var name by rememberSaveable {mutableStateOf("")}
    KitDialog(onDismissRequest=onDismiss,
        title={Text("New session",style=SettingsType.title)},text={
            Column(verticalArrangement=Arrangement.spacedBy(12.dp)) {
                Text("On $serverLabel",style=SettingsType.detail,color=MaterialTheme.colorScheme.onSurfaceVariant)
                OutlinedTextField(name,{name=it},Modifier.fillMaxWidth(),singleLine=true,
                    textStyle=SettingsType.row,label={Text("Name (optional)",style=SettingsType.detail)},
                    )
            }
        },confirmButton={Button(onClick={if(onCreate(name)) onDismiss()},enabled=enabled) {Text("Create",style=SettingsType.row)}},
        dismissButton={TextButton(onClick=onDismiss) {Text("Cancel",style=SettingsType.row)}})
}

@Composable internal fun EmptySessionContent(serverLabel: String,enabled: Boolean,onNew: ()->Unit,modifier: Modifier=Modifier) {
    Column(modifier.padding(24.dp),verticalArrangement=Arrangement.spacedBy(10.dp,Alignment.CenterVertically),horizontalAlignment=Alignment.CenterHorizontally) {
        Text("No sessions yet",style=SettingsType.title)
        Text("Start a session on $serverLabel.",style=SettingsType.body,color=MaterialTheme.colorScheme.onSurfaceVariant)
        FilledTonalButton(onClick=onNew,enabled=enabled) {
            ChromeSymbol(ChromeIcon.ADD,Modifier.size(17.dp));Spacer(Modifier.width(6.dp))
            Text("New session",style=SettingsType.row)
        }
    }
}

/** The viewed session ended while the phone was away: name what happened and offer a way back in. */
@Composable internal fun EndedSessionContent(serverLabel: String,enabled: Boolean,onBrowse: ()->Unit,onNew: ()->Unit,modifier: Modifier=Modifier) {
    Column(modifier.padding(24.dp),verticalArrangement=Arrangement.spacedBy(10.dp,Alignment.CenterVertically),horizontalAlignment=Alignment.CenterHorizontally) {
        Text("Session ended",style=SettingsType.title)
        Text("The session you were viewing is no longer on $serverLabel.",style=SettingsType.body,color=MaterialTheme.colorScheme.onSurfaceVariant)
        Row(horizontalArrangement=Arrangement.spacedBy(8.dp)) {
            FilledTonalButton(onClick=onBrowse,enabled=enabled) {
                ChromeSymbol(ChromeIcon.SESSIONS,Modifier.size(17.dp));Spacer(Modifier.width(6.dp))
                Text("Choose session",style=SettingsType.row)
            }
            Button(onClick=onNew,enabled=enabled) {
                ChromeSymbol(ChromeIcon.ADD,Modifier.size(17.dp));Spacer(Modifier.width(6.dp))
                Text("New session",style=SettingsType.row)
            }
        }
    }
}
