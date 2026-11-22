package org.telcoray.digitaldetox;

import android.content.Context;
import android.content.SharedPreferences;
import java.time.LocalDate;
import java.time.LocalDateTime;

/**
 * "Wait in the morning" (Detox tab): the first time the phone is unlocked each day, it locks
 * right away for the chosen minutes, on this phone only (see DetoxService.checkMorning). Meant
 * for the habit of picking the phone up in bed before doing anything else.
 */
final class MorningLock {
    static final int[] CHOICES = {15, 30, 45, 60};
    static final int DEFAULT_MINUTES = 30;
    /** The day starts at 5:00, as on Windows: unlocking the phone in the night doesn't count */
    static final int DAY_START_HOUR = 5;

    private MorningLock() {}

    private static SharedPreferences prefs(Context c) { return c.getSharedPreferences("morning", 0); }

    static boolean on(Context c) { return prefs(c).getBoolean("on", false); }

    static void setOn(Context c, boolean on) {
        SharedPreferences.Editor edit = prefs(c).edit().putBoolean("on", on);
        // Turned on while using the phone: it starts tomorrow, not right now
        if (on) edit.putString("done", day(LocalDateTime.now()).toString());
        edit.apply();
    }

    static int minutes(Context c) { return prefs(c).getInt("minutes", DEFAULT_MINUTES); }

    static void setMinutes(Context c, int minutes) { prefs(c).edit().putInt("minutes", minutes).apply(); }

    static void markDone(Context c, LocalDateTime now) { prefs(c).edit().putString("done", day(now).toString()).apply(); }

    static boolean due(Context c, LocalDateTime now) { return due(on(c), prefs(c).getString("done", ""), now); }

    /** Pure, for the tests: on, and not yet done in this day (which starts at 5:00) */
    static boolean due(boolean on, String done, LocalDateTime now) {
        return on && !day(now).toString().equals(done);
    }

    /** The day `now` belongs to: before 5:00 it's still the day before */
    static LocalDate day(LocalDateTime now) { return now.minusHours(DAY_START_HOUR).toLocalDate(); }

    /** Epoch milliseconds when today's day began (5:00) */
    static long dayStart(LocalDateTime now) {
        return day(now).atTime(DAY_START_HOUR, 0).atZone(java.time.ZoneId.systemDefault()).toInstant().toEpochMilli();
    }
}
