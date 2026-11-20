package org.telcoray.digitaldetox;

import android.content.Context;
import android.content.SharedPreferences;
import java.time.LocalDateTime;
import java.time.LocalDate;
import org.json.JSONException;
import org.json.JSONObject;

/**
 * The Pomodoro's settings on this phone: its lengths and rounds (the same "plan" the Windows app
 * sends), and "Start on its own each day": the first time the phone is unlocked after a chosen
 * time of day, a Pomodoro starts on this phone only (see DetoxService.checkDaily).
 */
final class PomodoroSettings {
    /** The usual start time: 8:00, when school or work begins, so the morning doesn't count */
    static final int DEFAULT_FROM = 8 * 60;

    private PomodoroSettings() {}

    private static SharedPreferences prefs(Context c) { return c.getSharedPreferences("pomodoro", 0); }

    /** The plan as the sync protocol has it: work, short_break, long_break, rounds, total */
    static JSONObject plan(Context c) {
        try { return new JSONObject(prefs(c).getString("plan", "{}")); }
        catch (JSONException e) { return new JSONObject(); }
    }

    static void savePlan(Context c, JSONObject plan) { prefs(c).edit().putString("plan", plan.toString()).apply(); }

    static boolean auto(Context c) { return prefs(c).getBoolean("auto", false); }

    static void setAuto(Context c, boolean on) { prefs(c).edit().putBoolean("auto", on).apply(); }

    /** Minutes after midnight */
    static int from(Context c) { return prefs(c).getInt("from", DEFAULT_FROM); }

    static void setFrom(Context c, int minutes) { prefs(c).edit().putInt("from", minutes).apply(); }

    /** Offered (or found running) today: not again until tomorrow */
    static void markDone(Context c, LocalDate today) { prefs(c).edit().putString("done", today.toString()).apply(); }

    static boolean due(Context c, LocalDateTime now) {
        return due(auto(c), prefs(c).getString("done", ""), from(c), now);
    }

    /** Pure, for the tests: on, not yet today, and past the chosen time */
    static boolean due(boolean auto, String done, int from, LocalDateTime now) {
        return auto && !now.toLocalDate().toString().equals(done) && now.getHour() * 60 + now.getMinute() >= from;
    }

    /** E.g. "08:00" */
    static String timeText(int minutes) {
        return String.format(java.util.Locale.ROOT, "%02d:%02d", minutes / 60, minutes % 60);
    }
}
