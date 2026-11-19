package dev.shulhi.rex

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
