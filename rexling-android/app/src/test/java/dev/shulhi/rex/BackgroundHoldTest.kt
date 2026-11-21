package dev.shulhi.rex

import org.junit.Assert.*
import org.junit.Test

class BackgroundHoldTest {
    @Test fun leavingTheScreenHoldsUntilTheLimit() {
        val hold=BackgroundHold(limit=45_000)
        assertTrue(hold.begin(1_000,interactive=true));assertTrue(hold.active)
        assertFalse(hold.expired(1_000,true));assertFalse(hold.expired(45_999,true))
        assertTrue(hold.expired(46_000,true))
    }
    @Test fun aDarkScreenIsNeverHeld() {
        val hold=BackgroundHold()
        assertFalse(hold.begin(1_000,interactive=false));assertFalse(hold.active)
        assertFalse(hold.expired(Long.MAX_VALUE))
    }
    @Test fun theScreenGoingDarkEndsAHoldEarly() {
        val hold=BackgroundHold(limit=45_000);hold.begin(0,true)
        assertFalse(hold.expired(5_000,interactive=true))
        assertTrue(hold.expired(5_000,interactive=false))
    }
    @Test fun returningInTimeLeavesNothingToExpire() {
        val hold=BackgroundHold(limit=45_000);hold.begin(0,true)
        assertFalse(hold.expired(10_000))
        hold.end()
        assertFalse(hold.active);assertFalse(hold.expired(90_000))
    }
    @Test fun returningAfterTheLimitCountsAsExpiredEvenIfNothingRanMeanwhile() {
        // A frozen process never reaches its own deadline check; the return has to notice.
        val hold=BackgroundHold(limit=45_000);hold.begin(0,true)
        assertTrue(hold.expired(120_000))
    }
    @Test fun theLimitStaysInsideTheMinuteAndroidAllows() {
        assertTrue(BackgroundHold.LIMIT_MS in 10_000 until 60_000)
    }
    @Test fun aNewHoldStartsItsOwnClock() {
        val hold=BackgroundHold(limit=45_000);hold.begin(0,true);hold.end()
        hold.begin(100_000,true)
        assertFalse(hold.expired(130_000));assertTrue(hold.expired(145_000))
    }
}
