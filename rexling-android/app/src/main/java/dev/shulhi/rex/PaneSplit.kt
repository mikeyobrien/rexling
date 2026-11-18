package dev.shulhi.rex

import androidx.compose.foundation.Canvas
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke

/** Rex names the axis of the layout, rather than the orientation of the divider. */
internal enum class PaneSplit(val label: String,val direction: String) {
    RIGHT("Split right","horizontal"), BELOW("Split below","vertical")
}

/** The split drawn as a frame with the new half filled in. */
@Composable internal fun SplitDiagram(split: PaneSplit,modifier: Modifier,color: Color) {
    Canvas(modifier) {
        val u=size.minDimension/24f
        val right=split==PaneSplit.RIGHT
        drawRoundRect(color.copy(alpha=.22f),Offset((if(right) 12f else 2f)*u,(if(right) 4f else 12f)*u),
            Size((if(right) 10f else 20f)*u,(if(right) 16f else 8f)*u),CornerRadius(1.5f*u))
        drawRoundRect(color,Offset(2f*u,4f*u),Size(20f*u,16f*u),CornerRadius(2f*u),style=Stroke(1.5f*u))
        drawLine(color,Offset((if(right) 12f else 2f)*u,(if(right) 4f else 12f)*u),
            Offset((if(right) 12f else 22f)*u,(if(right) 20f else 12f)*u),1.5f*u,StrokeCap.Round)
    }
}
