package org.telcoray.digitaldetox;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import java.time.LocalDateTime;
import org.junit.Test;

public final class PomodoroSettingsTest {
    private static final int EIGHT = 8 * 60, ALL = Schedule.EVERY_DAY, WEEKDAYS = 0b001_1111;

    private static LocalDateTime at(String text) { return LocalDateTime.parse(text); }

    @Test public void startsOnlyAfterTheChosenTime() {
        // 2026-11-20 is a Friday. Using the phone in the morning doesn't count.
        assertFalse(PomodoroSettings.due(true, ALL, "", EIGHT, at("2026-11-20T07:59")));
        assertTrue(PomodoroSettings.due(true, ALL, "", EIGHT, at("2026-11-20T08:00")));
        assertTrue(PomodoroSettings.due(true, ALL, "2026-11-19", EIGHT, at("2026-11-20T14:30")));
    }

    @Test public void onceADayAndOnlyWhenOn() {
        assertFalse(PomodoroSettings.due(true, ALL, "2026-11-20", EIGHT, at("2026-11-20T09:00")));
        assertFalse(PomodoroSettings.due(false, ALL, "", EIGHT, at("2026-11-20T09:00")));
    }

    @Test public void onlyOnTheChosenDays() {
        assertTrue(PomodoroSettings.due(true, WEEKDAYS, "", EIGHT, at("2026-11-20T09:00")));
        // Saturday
        assertFalse(PomodoroSettings.due(true, WEEKDAYS, "", EIGHT, at("2026-11-21T09:00")));
    }

    @Test public void showsTimesWithTwoDigits() {
        assertEquals("08:05", PomodoroSettings.timeText(8 * 60 + 5));
        assertEquals("13:30", PomodoroSettings.timeText(13 * 60 + 30));
    }
}
