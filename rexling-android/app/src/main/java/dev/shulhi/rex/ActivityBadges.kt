package dev.shulhi.rex

/** Unread presentation only; never marks records seen or changes notification delivery. */
internal data class ActivityBadges(val tabs: Map<String,Int> = emptyMap(),val elsewhere: Int=0)
internal object ActivityBadgePolicy {
    fun route(sources: List<ActivitySource>,server: ServerProfile,session: String,windows: List<WindowItem>): ActivityBadges {
        val panes=windows.flatMap {w->w.blocks.map {it.id to w.id}}.toMap()
        val counts=mutableMapOf<String,Int>();var elsewhere=0
        for(source in sources.filter {it.connected}) {
            val local=source.server.id==server.id && source.server.endpoint==server.endpoint
            for(record in source.items.distinctBy {it.key}.filter {it.notifiable && it.key !in source.seen}) {
                val tab=if(local && session.isNotBlank() && record.session==session) panes[record.block] else null
                if(tab!=null) counts[tab]=(counts[tab] ?: 0)+1 else elsewhere++
            }
        }
        return ActivityBadges(counts,elsewhere)
    }
    /** Per session key: whether anything in it is unread (needs you) or working. Same rule as the tab badges. */
    fun sessions(sources: List<ActivitySource>): Map<String,SessionActivity> {
        val result=mutableMapOf<String,SessionActivity>()
        for(source in sources.filter {it.connected}) for(record in source.items.distinctBy {it.key}) {
            val attention=record.notifiable && record.key !in source.seen
            val working=record.state==ActivityState.WORKING
            if(!attention && !working) continue
            val key=sessionKey(source.server.id,record.session)
            val old=result[key]
            result[key]=SessionActivity(attention || old?.attention==true,working || old?.working==true)
        }
        return result
    }
}
internal data class SessionActivity(val attention: Boolean,val working: Boolean)
