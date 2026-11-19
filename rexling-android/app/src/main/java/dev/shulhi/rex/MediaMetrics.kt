package dev.shulhi.rex

import android.os.SystemClock
import android.util.Log

/**
 * Timing for pictures streamed into a pane with Kitty graphics, on the RexlingMedia logcat tag.
 * Off unless enabled: adb shell setprop log.tag.RexlingMedia DEBUG
 *   picture t=<ms> images=<n> frame=<k>   a draw that showed picture content not drawn before; frame is
 *                               the number the sender wrote as text under the picture, or -1
 *   touch latency=<ms>          finger up until the next such draw
 *   stages n=<updates> read=<ms> decode=<ms> apply=<ms>   mean cost per update over the last second:
 *                               copying a packet out of the engine, decoding it, applying it on the main thread
 * Times end at onDraw, so they leave out RenderThread and the compositor (about one frame).
 */
internal class MediaMetrics {
    private var shown=0L
    private var touchedAt=0L
    fun touched(eventTime: Long) {if(Log.isLoggable(TAG,Log.DEBUG)) touchedAt=eventTime}
    fun drawn(screen: TerminalScreen) {
        if(screen.images.isEmpty() || !Log.isLoggable(TAG,Log.DEBUG)) return
        var signature=1L
        for(placement in screen.images) signature=signature*31+placement.image.id*131+placement.image.generation
        if(signature==shown) return
        shown=signature
        val now=SystemClock.uptimeMillis()
        val stamp=screen.lines.firstOrNull()?.takeWhile {it.text.length==1 && it.text[0].isDigit()}?.joinToString("") {it.text}
        Log.d(TAG,"picture t=$now images=${screen.images.size} frame=${stamp?.toIntOrNull() ?: -1}")
        if(touchedAt!=0L) {Log.d(TAG,"touch latency=${now-touchedAt}");touchedAt=0}
    }
    companion object {
        const val TAG="RexlingMedia"
        private val totals=LongArray(3)
        private var count=0
        private var reportAt=0L
        /** Called from the update loop with the three stage durations of one update that carried pictures. */
        fun stages(read: Long,decode: Long,apply: Long) {
            if(!Log.isLoggable(TAG,Log.DEBUG)) return
            synchronized(totals) {
                totals[0]+=read;totals[1]+=decode;totals[2]+=apply;count++
                val now=SystemClock.uptimeMillis()
                if(now<reportAt) return
                Log.d(TAG,"stages n=%d read=%.1f decode=%.1f apply=%.1f".format(count,totals[0]/1e6/count,totals[1]/1e6/count,totals[2]/1e6/count))
                totals.fill(0);count=0;reportAt=now+1000
            }
        }
    }
}
