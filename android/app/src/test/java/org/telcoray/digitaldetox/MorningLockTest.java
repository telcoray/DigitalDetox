package org.telcoray.digitaldetox;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import java.time.LocalDate;
import java.time.LocalDateTime;
import org.junit.Test;

public class MorningLockTest {
    @Test public void theNightBelongsToTheDayBefore() {
        assertEquals(LocalDate.of(2026, 11, 21), MorningLock.day(LocalDateTime.of(2026, 11, 22, 4, 59)));
        assertEquals(LocalDate.of(2026, 11, 22), MorningLock.day(LocalDateTime.of(2026, 11, 22, 5, 0)));
    }

    @Test public void onceADay() {
        LocalDateTime morning = LocalDateTime.of(2026, 11, 22, 7, 30);
        assertTrue(MorningLock.due(true, "2026-11-21", morning));
        assertFalse(MorningLock.due(true, "2026-11-22", morning));
        assertFalse(MorningLock.due(false, "2026-11-21", morning));
    }

    @Test public void unlockingInTheNightDoesNotUseUpTheMorning() {
        // Done yesterday evening; at 3:00 it's still yesterday, at 7:00 it's due
        assertFalse(MorningLock.due(true, "2026-11-21", LocalDateTime.of(2026, 11, 22, 3, 0)));
        assertTrue(MorningLock.due(true, "2026-11-21", LocalDateTime.of(2026, 11, 22, 7, 0)));
    }
}
