package dev.shulhi.rex

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.FastOutSlowInEasing
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.imeAnimationSource
import androidx.compose.foundation.layout.imeAnimationTarget
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import kotlin.math.roundToInt

/** Rows of a card that has just opened follow each other in, this far apart. */
internal const val RowStaggerMillis=20L
/** How long a card counts as opening: rows composed later (by scrolling) simply appear. */
internal const val ArrivalMillis=400L

/** How far the keyboard has travelled: 0 hidden, 1 fully open, in between while it animates. */
internal fun keyboardProgress(now: Int,source: Int,target: Int): Float {
    val full=maxOf(source,target)
    return if(full<=0) (if(now>0) 1f else 0f) else (now.toFloat()/full).coerceIn(0f,1f)
}

/**
 * Grows with the keyboard's own animation, so what sits on the keyboard arrives with it instead of
 * popping in on its first frame. Read during layout: it never recomposes.
 */
@OptIn(ExperimentalLayoutApi::class)
@Composable internal fun Modifier.riseWithKeyboard(): Modifier {
    val ime=WindowInsets.ime;val source=WindowInsets.imeAnimationSource;val target=WindowInsets.imeAnimationTarget
    return clipToBounds().layout {measurable,constraints->
        val child=measurable.measure(constraints)
        val progress=keyboardProgress(ime.getBottom(this),source.getBottom(this),target.getBottom(this))
        layout(child.width,(child.height*progress).roundToInt()) {child.place(0,0)}
    }
}

/** A row following the card it belongs to: [order] rows after the first, rising a little as it fades in. */
@Composable internal fun Modifier.arriving(order: Int?): Modifier {
    if(order==null) return this
    val enter=remember {Animatable(0f)}
    LaunchedEffect(Unit) {delay(order*RowStaggerMillis);enter.animateTo(1f,spring(dampingRatio=.82f,stiffness=420f))}
    val rise=with(LocalDensity.current) {14.dp.toPx()}
    return graphicsLayer {alpha=enter.value.coerceIn(0f,1f);translationY=(1f-enter.value)*rise}
}

/**
 * Which way a tab change travels, and how far along it is. Going to a later tab the content comes in
 * from the right, to an earlier one from the left; a change of session is not travel and just cuts.
 */
@Stable internal class TabTravel {
    val progress=Animatable(1f)
    var direction by mutableIntStateOf(1)
        private set
    private var from: Triple<String,String,Int>?=null
    suspend fun arrive(session: String,tab: String,index: Int) {
        val last=from;from=Triple(session,tab,index)
        if(last==null || tab.isBlank() || last.second.isBlank() || last.first!=session || last.second==tab) return
        direction=if(index>=last.third) 1 else -1
        progress.snapTo(0f);progress.animateTo(1f,tween(220,easing=FastOutSlowInEasing))
    }
}

/** The arriving tab's content: a short slide from the side it came from. */
internal fun Modifier.travelling(travel: TabTravel): Modifier=graphicsLayer {
    val p=travel.progress.value
    translationX=(1f-p)*travel.direction*size.width*.14f;alpha=.25f+.75f*p
}
