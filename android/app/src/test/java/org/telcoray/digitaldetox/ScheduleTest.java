package org.telcoray.digitaldetox;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNull;

import java.time.LocalDateTime;
import java.util.List;
import org.junit.Test;

public final class ScheduleTest {
    private static LocalDateTime at(String text) { return LocalDateTime.parse(text); }

    private static final List<Schedule.Time> ONE_PM = List.of(new Schedule.Time(13, 0, 30, Schedule.EVERY_DAY));

    @Test public void warnsAMinuteBeforeAndStaysDueALittleAfter() {
        assertNull(Schedule.due(ONE_PM, at("2026-11-20T12:58:30"), ""));
        assertEquals(at("2026-11-20T13:00"), Schedule.due(ONE_PM, at("2026-11-20T12:59:10"), "").at());
        assertEquals(30, Schedule.due(ONE_PM, at("2026-11-20T13:01"), "").minutes());
        // The phone was off: too late now
        assertNull(Schedule.due(ONE_PM, at("2026-11-20T13:03"), ""));
    }

    @Test public void notAgainOnceHandled() {
        assertNull(Schedule.due(ONE_PM, at("2026-11-20T13:00:30"), "2026-11-20T13:00"));
    }

    @Test public void acrossMidnight() {
        List<Schedule.Time> midnight = List.of(new Schedule.Time(0, 0, 10, Schedule.EVERY_DAY));
        assertEquals(at("2026-11-21T00:00"), Schedule.due(midnight, at("2026-11-20T23:59:30"), "").at());
        assertEquals(at("2026-11-21T00:00"), Schedule.due(midnight, at("2026-11-21T00:01"), "").at());
    }

    @Test public void onlyOnTheChosenDays() {
        // Weekdays only; 2026-11-21 is a Saturday
        List<Schedule.Time> weekdays = List.of(new Schedule.Time(13, 0, 30, 0b001_1111));
        assertNull(Schedule.due(weekdays, at("2026-11-21T12:59:30"), ""));
        assertEquals("Weekdays", Schedule.daysText(0b001_1111));
        assertEquals("Mo We Fr", Schedule.daysText(0b001_0101));
    }
}
