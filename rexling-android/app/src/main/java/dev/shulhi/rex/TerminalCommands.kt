package dev.shulhi.rex

import org.json.JSONObject

/** An open palette never follows focus to another terminal, even after a reconnect. */
internal data class TerminalCommandTarget(val epoch: Long,val navigationId: Long,val session: String,val tab: String,val pane: String) {
    fun matches(state: AppState,pending: Boolean): Boolean = !pending && state.connected && !state.creating &&
        state.connectionEpoch==epoch && state.navigationId==navigationId && state.selectedSession==session && state.selectedBlock==pane &&
        state.windows.any {it.id==tab && it.blocks.any {block->block.id==pane}}

    companion object {
        fun capture(state: AppState,pending: Boolean): TerminalCommandTarget? {
            if(state.selectedSession.isBlank() || state.selectedBlock.isBlank()) return null
            val tab=state.windows.firstOrNull {it.blocks.any {block->block.id==state.selectedBlock}} ?: return null
            return TerminalCommandTarget(state.connectionEpoch,state.navigationId,state.selectedSession,tab.id,state.selectedBlock).takeIf {it.matches(state,pending)}
        }
    }
}

internal enum class TerminalCommand(val label: String,val scope: String,val icon: ChromeIcon) {
    SPLIT("Split pane…","Pane",ChromeIcon.SPLIT),
    COPY("Copy viewport","Pane",ChromeIcon.COPY),
    RENAME("Rename tab…","Tab",ChromeIcon.EDIT),
    FORWARDS("Port forwarding…","Session",ChromeIcon.SERVER);

    companion object {
        fun matching(query: String,canCopy: Boolean): List<TerminalCommand> {
            val words=query.trim().split(Regex("\\s+")).filter {it.isNotBlank()}
            return entries.filter {command->(canCopy || command!=COPY) && words.all {
                "${command.label} ${command.scope}".contains(it,ignoreCase=true)
            }}
        }
    }
}

data class ServerCommand(val name: String,val title: String,val category: String,val keywords: String,val requiresArguments: Boolean) {
    fun matches(query: String): Boolean = query.trim().split(Regex("\\s+")).all {
        "$title $category $keywords $name".contains(it,ignoreCase=true)
    }

    companion object {
        fun parse(row: JSONObject): ServerCommand = ServerCommand(row.getString("name"),row.getString("title"),
            row.optString("category"),row.optString("keywords"),
            (row.optJSONObject("args")?.optJSONArray("required")?.length() ?: 0)>0)
    }
}

data class ServerCommandCatalog(val actions: List<ServerCommand>,val error: String) {
    companion object {
        fun parse(row: JSONObject) = ServerCommandCatalog(
            row.optJSONArray("actions")?.let {list->List(list.length()) {ServerCommand.parse(list.getJSONObject(it))}}.orEmpty(),
            row.optString("error"))
    }
}

data class ServerCommandRun(val id: Long,val running: Boolean,val message: String,val error: String,val warning: String) {
    companion object {
        fun parse(row: JSONObject) = ServerCommandRun(row.getLong("id"),row.optBoolean("running"),row.optString("message"),row.optString("error"),row.optString("warning"))
    }
}
