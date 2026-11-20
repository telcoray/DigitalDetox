package org.telcoray.digitaldetox;

import android.content.Context;
import java.time.DayOfWeek;
import java.time.LocalDateTime;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * The Schedule tab: detoxes at set times, on the days chosen for each, like the Windows app's
 * (src/schedule.rs). A notification warns a minute before; then this phone locks, this phone
 * only. Days are bits from Monday (bit 0) to Sunday (bit 6), as on Windows.
 */
final class Schedule {
    static final int MAX = 4;
    static final int EVERY_DAY = 0b111_1111;
    static final long WARNING = 60_000;
    /** A time missed by more than this (the phone was off) is let go */
    static final long LATE_BY_AT_MOST = 120_000;
    static final String[] DAY_NAMES = {"Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"};

    record Time(int hour, int minute, int minutes, int days) {
        boolean on(DayOfWeek day) { return (days >> (day.getValue() - 1) & 1) == 1; }
    }

    /** A set time's next occurrence, and how long it locks for */
    record Due(LocalDateTime at, int minutes) {
        String key() { return at.toString(); }
    }

    private Schedule() {}

    static List<Time> load(Context c) {
        List<Time> times = new ArrayList<>();
        try {
            JSONArray array = new JSONArray(c.getSharedPreferences("schedule", 0).getString("times", "[]"));
            for (int i = 0; i < array.length(); i++) {
                JSONObject t = array.getJSONObject(i);
                times.add(new Time(t.getInt("hour"), t.getInt("minute"), t.getInt("minutes"), t.optInt("days", EVERY_DAY)));
            }
        } catch (JSONException ignored) {
            // A broken list is an empty one
        }
        return times;
    }

    static void save(Context c, List<Time> times) {
        JSONArray array = new JSONArray();
        for (Time t : times) {
            array.put(DetoxService.obj("hour", t.hour(), "minute", t.minute(), "minutes", t.minutes(), "days", t.days()));
        }
        c.getSharedPreferences("schedule", 0).edit().putString("times", array.toString()).apply();
    }

    /** The occurrence that is a minute away or under way now, unless it's `handled` (warned and done, or skipped) */
    static Due due(List<Time> times, LocalDateTime now, String handled) {
        Due best = null;
        for (Time t : times) {
            // Yesterday's, for just after midnight; tomorrow's, for its warning just before
            for (int day = -1; day <= 1; day++) {
                LocalDateTime at = now.toLocalDate().plusDays(day).atTime(t.hour(), t.minute());
                if (!t.on(at.getDayOfWeek())) continue;
                long until = java.time.Duration.between(now, at).toMillis();
                if (until > WARNING || until < -LATE_BY_AT_MOST) continue;
                Due candidate = new Due(at, t.minutes());
                if (candidate.key().equals(handled)) continue;
                if (best == null || at.isBefore(best.at())) best = candidate;
            }
        }
        return best;
    }

    /** E.g. "Every day", "Weekdays", "Mo We Fr" */
    static String daysText(int days) {
        if ((days & EVERY_DAY) == EVERY_DAY) return "Every day";
        if (days == 0b001_1111) return "Weekdays";
        if (days == 0b110_0000) return "Weekends";
        StringBuilder text = new StringBuilder();
        for (int i = 0; i < 7; i++) {
            if ((days >> i & 1) == 1) text.append(text.length() == 0 ? "" : " ").append(DAY_NAMES[i]);
        }
        return text.length() == 0 ? "No days" : text.toString();
    }
}
