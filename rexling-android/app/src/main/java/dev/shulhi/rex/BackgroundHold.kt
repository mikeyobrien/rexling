package dev.shulhi.rex

import android.app.job.JobInfo
import android.app.job.JobParameters
import android.app.job.JobScheduler
import android.app.job.JobService
import android.content.ComponentName
import android.content.Context
import android.os.Build
import android.util.Log

/**
 * How long Rexling stays connected after it leaves the screen, so a short trip to another app needs no
 * reconnect. Connections and pane claims are kept as they are for [limit] and paused after that. A dark screen
 * ends the hold at once: the phone was put down, and another client should get its pane size back.
 */
internal class BackgroundHold(private val limit: Long=LIMIT_MS) {
    private var until: Long?=null
    val active: Boolean get()=until!=null
    /** The app left the screen at [now]. False when it should pause at once instead. */
    fun begin(now: Long,interactive: Boolean): Boolean {until=if(interactive) now+limit else null;return active}
    /**
     * Whether a running hold has to stop. Also true for an app that returns after the limit without having
     * paused: Android had it frozen, and connections it could not serve meanwhile cannot be trusted.
     */
    fun expired(now: Long,interactive: Boolean=true): Boolean = until?.let {now>=it || !interactive} ?: false
    fun end() {until=null}
    companion object {
        /** Under a minute: Android promises that much running time to the job that keeps the process alive meanwhile. */
        const val LIMIT_MS=45_000L
    }
}

private const val TAG="RexlingHold"
/** Why a hold began or ended. Off unless: adb shell setprop log.tag.RexlingHold DEBUG */
internal fun holdLog(message: String) {if(Log.isLoggable(TAG,Log.DEBUG)) Log.d(TAG,message)}

/**
 * Runs for as long as a hold lasts and does nothing itself. Android freezes a backgrounded app about ten
 * seconds after a second app is opened over it. A frozen app can neither serve its connections nor give them
 * up when the hold ends; a running job keeps the process out of that state.
 */
class BackgroundHoldJob: JobService() {
    override fun onStartJob(params: JobParameters)=true
    override fun onStopJob(params: JobParameters)=false
    companion object {
        private const val ID=7200
        fun start(context: Context) {
            // Before Android 12 backgrounded apps were not frozen, and there were no expedited jobs to ask for.
            if(Build.VERSION.SDK_INT<31) return
            runCatching {context.getSystemService(JobScheduler::class.java).schedule(
                JobInfo.Builder(ID,ComponentName(context,BackgroundHoldJob::class.java)).setExpedited(true).build())}
        }
        fun stop(context: Context) {runCatching {context.getSystemService(JobScheduler::class.java).cancel(ID)}}
    }
}
