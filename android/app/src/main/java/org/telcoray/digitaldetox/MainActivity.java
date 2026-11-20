package org.telcoray.digitaldetox;

import android.app.Activity;
import android.app.AlertDialog;
import android.app.admin.DevicePolicyManager;
import android.content.ComponentName;
import android.content.Intent;
import android.content.res.ColorStateList;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.graphics.drawable.RippleDrawable;
import android.os.Build;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.SystemClock;
import android.provider.Settings;
import android.text.Editable;
import android.text.InputFilter;
import android.text.InputType;
import android.text.TextWatcher;
import android.view.Gravity;
import android.view.View;
import android.view.WindowInsets;
import android.widget.Button;
import android.widget.EditText;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.TextView;
import android.widget.Toast;
import java.util.ArrayList;
import java.util.List;
import org.json.JSONObject;

public final class MainActivity extends Activity {
    private static final int BG = 0xff1a1a2e, PANEL = 0xff242437, EDGE = 0xff34495e;
    private static final int WHITE = 0xffecf0f1, MUTED = 0xffadbcc4, BLUE = 0xff3498db;
    private static final int RED = 0xffed1d25, GREEN = 0xff90ee90;
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final List<LinearLayout> pages = new ArrayList<>();
    private final List<Button> tabs = new ArrayList<>();
    private final List<Button> presets = new ArrayList<>();
    private static final int[] PRESET_MINUTES = {5, 15, 25, 45, 60};
    private ScrollView scroll;
    private TextView access, unlock, countdown, phase, rounds, peers, pairingCode, syncStatus;
    private Button pause, stop, startPomodoro, sync, changeTimes, fromButton;
    private TextView planSummary, dailyHint;
    private android.widget.Switch daily;
    private EditText minutes;
    private int selected;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        LinearLayout root = column();
        root.setBackgroundColor(BG);
        root.setOnApplyWindowInsetsListener((v, insets) -> {
            var bars = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
            int bottom = Math.max(bars.bottom, insets.getInsets(WindowInsets.Type.ime()).bottom);
            v.setPadding(bars.left + dp(20), bars.top, bars.right + dp(20), bottom);
            return insets;
        });
        setContentView(root);
        LinearLayout header = row();
        header.setPadding(0, dp(8), 0, dp(12));
        ImageView logo = new ImageView(this);
        logo.setImageResource(R.drawable.logo);
        header.addView(logo, dp(32), dp(32));
        TextView brand = label("Digital Detox", 23, WHITE);
        brand.setTypeface(Typeface.DEFAULT, Typeface.BOLD);
        brand.setPadding(dp(10), 0, 0, 0);
        header.addView(brand, new LinearLayout.LayoutParams(0, -2, 1));
        header.addView(button("Settings", PANEL, this::settings));
        root.addView(header);

        LinearLayout nav = row();
        nav.setPadding(dp(4), dp(4), dp(4), dp(4));
        nav.setBackground(shape(PANEL, 17, EDGE));
        String[] names = {"Detox", "Pomodoro", "Sync"};
        for (int i = 0; i < names.length; i++) {
            int index = i;
            Button tab = button(names[i], PANEL, () -> select(index));
            nav.addView(tab, new LinearLayout.LayoutParams(0, dp(48), 1));
            tabs.add(tab);
        }
        root.addView(nav);

        scroll = new ScrollView(this);
        scroll.setFillViewport(true);
        LinearLayout stack = column();
        scroll.addView(stack);
        root.addView(scroll, new LinearLayout.LayoutParams(-1, 0, 1));
        pages.add(detoxPage());
        pages.add(pomodoroPage());
        pages.add(syncPage());
        for (LinearLayout page : pages) stack.addView(page);
        select(state == null ? 0 : state.getInt("tab"));
        startForegroundService(new Intent(this, DetoxService.class));
    }

    private LinearLayout detoxPage() {
        LinearLayout page = page("Take a break", "Disconnect for a while and come back refreshed.");
        LinearLayout card = card(page);
        add(card, label("Duration · minutes", 15, MUTED), 0);
        minutes = numberField("25", "1–999", 3);
        minutes.setTextSize(46);
        minutes.setSelectAllOnFocus(true);
        add(card, minutes, 10);
        LinearLayout presetRow = row();
        for (int value : PRESET_MINUTES) {
            Button preset = button(Integer.toString(value), BG, () -> minutes.setText(Integer.toString(value)));
            LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(0, dp(48), 1);
            params.setMargins(dp(2), 0, dp(2), 0);
            presetRow.addView(preset, params);
            presets.add(preset);
        }
        add(card, presetRow, 12);
        unlock = label("", 15, MUTED);
        unlock.setGravity(Gravity.CENTER);
        add(card, unlock, 12);
        minutes.addTextChangedListener(new TextWatcher() {
            @Override public void beforeTextChanged(CharSequence text, int start, int count, int after) {}
            @Override public void onTextChanged(CharSequence text, int start, int before, int count) {
                updateUnlock();
                updatePresets();
            }
            @Override public void afterTextChanged(Editable text) {}
        });
        add(page, button("Start detox", BLUE, this::startDetox), 14);
        access = label("", 14, MUTED);
        add(page, access, 12);
        Button setup = button("Set up blocking", PANEL, this::blockingPermission);
        add(page, setup, 8);
        access.setTag(setup);
        updateUnlock();
        updatePresets();
        return page;
    }

    private LinearLayout pomodoroPage() {
        LinearLayout page = page("Focus in rounds", "25 min focus  ·  5 min break  ·  15 min long break");
        LinearLayout card = card(page);
        countdown = label("25:00", 54, WHITE);
        countdown.setTypeface(Typeface.MONOSPACE);
        countdown.setGravity(Gravity.CENTER);
        add(card, countdown, 0);
        phase = label("Until your 5-minute break", 16, MUTED);
        phase.setGravity(Gravity.CENTER);
        add(card, phase, 4);
        rounds = label("Round 1 of 4", 14, 0xffffa3a7);
        rounds.setGravity(Gravity.CENTER);
        add(card, rounds, 16);
        startPomodoro = button("Start Pomodoro", RED, () -> {
            if (ready()) confirm("Start Pomodoro? This phone will block during breaks.", "Start",
                () -> publish(DetoxService.obj("kind", "pomodoro", "start", Session.now(), "plan", PomodoroSettings.plan(this))));
        });
        add(page, startPomodoro, 14);
        planSummary = (TextView) page.getChildAt(1);
        LinearLayout actions = row();
        pause = button("Pause", PANEL, this::togglePause);
        stop = button("Stop", PANEL, () -> {
            DetoxService s = service();
            if (s != null && !s.locked() && "pomodoro".equals(s.shared.optString("kind"))) {
                s.update(DetoxService.obj("kind", "idle"));
            }
        });
        actions.addView(pause, new LinearLayout.LayoutParams(0, -2, 1));
        LinearLayout.LayoutParams right = new LinearLayout.LayoutParams(0, -2, 1);
        right.leftMargin = dp(8);
        actions.addView(stop, right);
        add(page, actions, 12);
        changeTimes = button("Change times", PANEL, this::editPlan);
        add(page, changeTimes, 8);

        // "Start on its own each day", see PomodoroSettings and DetoxService.checkDaily
        LinearLayout card2 = card(page);
        LinearLayout switchRow = row();
        switchRow.addView(label("Start on its own each day", 16, WHITE), new LinearLayout.LayoutParams(0, -2, 1));
        daily = new android.widget.Switch(this);
        daily.setChecked(PomodoroSettings.auto(this));
        daily.setOnCheckedChangeListener((v, on) -> {
            PomodoroSettings.setAuto(this, on);
            updateDaily();
        });
        switchRow.addView(daily);
        add(card2, switchRow, 0);
        LinearLayout fromRow = row();
        fromRow.addView(label("From", 16, WHITE), new LinearLayout.LayoutParams(0, -2, 1));
        fromButton = button("", BG, this::chooseFrom);
        fromRow.addView(fromButton, new LinearLayout.LayoutParams(dp(110), -2));
        add(card2, fromRow, 10);
        dailyHint = label("", 14, MUTED);
        add(card2, dailyHint, 10);
        updateDaily();
        return page;
    }

    private void updateDaily() {
        int from = PomodoroSettings.from(this);
        fromButton.setText(PomodoroSettings.timeText(from));
        fromButton.setEnabled(daily.isChecked());
        fromButton.setAlpha(daily.isChecked() ? 1f : 0.5f);
        dailyHint.setText("The first time you unlock the phone after " + PomodoroSettings.timeText(from)
            + ", a notification counts down 30 seconds, then a Pomodoro starts on this phone only."
            + " Using the phone earlier doesn't count.");
    }

    private void chooseFrom() {
        int from = PomodoroSettings.from(this);
        new android.app.TimePickerDialog(this, (picker, hour, minute) -> {
            PomodoroSettings.setFrom(this, hour * 60 + minute);
            updateDaily();
        }, from / 60, from % 60, android.text.format.DateFormat.is24HourFormat(this)).show();
    }

    /** "Change times": the same choices as in the Windows app, with the same limits */
    private void editPlan() {
        Session.Plan plan = savedPlan();
        String[] names = {"Focus · minutes", "Short break · minutes", "Long break · minutes",
            "Rounds before the long break", "Total rounds, then stop (empty = until stopped)"};
        String[] keys = {"work", "short_break", "long_break", "rounds", "total"};
        int[] min = {1, 1, 1, 1, 0}, max = {180, 120, 120, 8, 50};
        long[] values = {plan.work() / 60000, plan.shortBreak() / 60000, plan.longBreak() / 60000, plan.rounds(), plan.total()};
        int[] usual = {25, 5, 15, 4, 0};
        EditText[] fields = new EditText[5];
        LinearLayout box = column();
        box.setPadding(dp(22), dp(8), dp(22), 0);
        for (int i = 0; i < 5; i++) {
            add(box, label(names[i], 14, MUTED), i == 0 ? 0 : 10);
            fields[i] = numberField(i == 4 && values[i] == 0 ? "" : Long.toString(values[i]), i == 4 ? "until stopped" : "", 3);
            add(box, fields[i], 4);
        }
        ScrollView scrollBox = new ScrollView(this);
        scrollBox.addView(box);
        AlertDialog dialog = new AlertDialog.Builder(this).setTitle("Pomodoro times").setView(scrollBox)
            .setPositiveButton("Save", null).setNeutralButton("Usual", null).setNegativeButton("Cancel", null).create();
        dialog.setOnShowListener(d -> {
            // "Usual" fills in 25 / 5 / 15, 4 rounds, without closing
            dialog.getButton(AlertDialog.BUTTON_NEUTRAL).setOnClickListener(v -> {
                for (int i = 0; i < 5; i++) fields[i].setText(i == 4 ? "" : Integer.toString(usual[i]));
            });
            dialog.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener(v -> {
                JSONObject saved = new JSONObject();
                for (int i = 0; i < 5; i++) {
                    String text = fields[i].getText().toString().trim();
                    int value;
                    try { value = text.isEmpty() && i == 4 ? 0 : Integer.parseInt(text); }
                    catch (NumberFormatException e) { value = -1; }
                    if (value < min[i] || value > max[i]) {
                        fields[i].setError("A number from " + min[i] + " to " + max[i]);
                        return;
                    }
                    try { saved.put(keys[i], value); }
                    catch (org.json.JSONException e) { throw new IllegalStateException(e); }
                }
                PomodoroSettings.savePlan(this, saved);
                dialog.dismiss();
            });
        });
        dialog.show();
    }

    /** The lengths a Pomodoro started here would have */
    private Session.Plan savedPlan() {
        return Session.Plan.of(DetoxService.obj("plan", PomodoroSettings.plan(this)));
    }

    private LinearLayout syncPage() {
        LinearLayout page = page("Lock your devices together", "Pair devices on the same local network.");
        LinearLayout card = card(page);
        add(card, label("Paired devices", 16, WHITE), 0);
        peers = label("No paired devices online yet", 17, MUTED);
        add(card, peers, 12);
        sync = button("Enable Sync", PANEL, this::toggleSync);
        add(card, sync, 14);
        pairingCode = label("", 34, BLUE);
        pairingCode.setTypeface(Typeface.MONOSPACE);
        pairingCode.setGravity(Gravity.CENTER);
        add(card, pairingCode, 12);
        syncStatus = label("", 14, MUTED);
        add(card, syncStatus, 8);
        add(page, button("Pair a device", BLUE, () -> {
            if (ready()) confirm("Pair this phone and allow shared sessions?", "Continue", () -> {
                if (ready()) service().invite();
            });
        }), 14);
        add(page, button("Enter a code", PANEL, this::enterCode), 8);
        return page;
    }

    private void enterCode() {
        if (!ready()) return;
        EditText code = numberField("", "Six-digit code", 6);
        LinearLayout box = column();
        box.setPadding(dp(22), dp(8), dp(22), 0);
        box.addView(code);
        AlertDialog dialog = new AlertDialog.Builder(this).setTitle("Enter a code")
            .setMessage("Use the code shown on your other device. Pairing allows shared sessions.")
            .setView(box).setPositiveButton("Pair", null).setNegativeButton("Cancel", null).create();
        dialog.setOnShowListener(d -> dialog.getButton(AlertDialog.BUTTON_POSITIVE).setOnClickListener(v -> {
            String value = code.getText().toString();
            if (!value.matches("[0-9]{6}")) {
                code.setError("Enter all six digits");
            } else if (ready()) {
                service().join(value);
                dialog.dismiss();
            }
        }));
        dialog.show();
    }

    private void startDetox() {
        if (!ready()) return;
        int value = duration();
        if (value < 1) {
            minutes.setError("Enter 1–999 minutes");
            return;
        }
        confirm("Block this phone for " + value + " minutes?", "Start",
            () -> publish(DetoxService.obj("kind", "detox", "ends_at", Session.now() + value * 60000L)));
    }

    private int duration() {
        try { return Integer.parseInt(minutes.getText().toString()); }
        catch (NumberFormatException e) { return 0; }
    }

    private void updateUnlock() {
        int value = duration();
        if (value < 1) {
            unlock.setText("Choose a duration from 1 to 999 minutes");
            return;
        }
        String time = android.text.format.DateFormat.getTimeFormat(this)
            .format(new java.util.Date(Session.now() + value * 60000L));
        unlock.setText("Unlocks at " + time);
    }

    private void updatePresets() {
        int value = duration();
        for (int i = 0; i < presets.size(); i++) {
            boolean active = PRESET_MINUTES[i] == value;
            Button preset = presets.get(i);
            preset.setSelected(active);
            preset.setBackground(ripple(active ? 0xff253951 : BG, 13, active ? BLUE : EDGE));
        }
    }

    private void togglePause() {
        DetoxService s = service();
        if (s == null || s.locked() || !"pomodoro".equals(s.shared.optString("kind"))) return;
        long start = s.shared.optLong("start");
        long now = Session.now();
        JSONObject updated = s.shared.has("paused_at")
            ? DetoxService.obj("kind", "pomodoro", "start", start + Math.max(0, now - s.shared.optLong("paused_at")))
            : DetoxService.obj("kind", "pomodoro", "start", start, "paused_at", now);
        // The lengths chosen where it was started go along, or the other devices would lose them
        JSONObject plan = s.shared.optJSONObject("plan");
        if (plan != null) {
            try {
                updated.put("plan", plan);
            } catch (org.json.JSONException e) {
                throw new IllegalStateException(e);
            }
        }
        s.update(updated);
    }

    private void toggleSync() {
        DetoxService s = service();
        if (s == null || s.locked()) return;
        if (s.syncEnabled) {
            s.setSync(false);
        } else if (ready()) {
            confirm("Allow paired devices to start sessions on this phone?", "Enable",
                () -> { if (ready()) service().setSync(true); });
        }
    }

    private void settings() {
        String[] options = {"Blocking permission", "Notifications", "Background battery settings",
            "Optional uninstall protection", "Forget paired devices", "Stop background service"};
        new AlertDialog.Builder(this).setTitle("Settings").setItems(options, (d, which) -> {
            switch (which) {
                case 0 -> blockingPermission();
                case 1 -> {
                    if (Build.VERSION.SDK_INT >= 33) {
                        requestPermissions(new String[]{android.Manifest.permission.POST_NOTIFICATIONS}, 1);
                    } else {
                        startActivity(new Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS)
                            .putExtra(Settings.EXTRA_APP_PACKAGE, getPackageName()));
                    }
                }
                case 2 -> new AlertDialog.Builder(this).setTitle("Run in background")
                    .setMessage("On HONOR, open Settings → Apps → App launch. Turn off automatic management for Digital Detox, then allow Auto-launch, Secondary launch and Run in background.")
                    .setPositiveButton("Battery settings", (a, b) ->
                        startActivity(new Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS)))
                    .setNegativeButton("Close", null).show();
                case 3 -> confirm("Android requires deactivating a device administrator before uninstalling it. Digital Detox requests no password, wipe or remote-lock policies.", "Continue",
                    () -> startActivity(new Intent(DevicePolicyManager.ACTION_ADD_DEVICE_ADMIN)
                        .putExtra(DevicePolicyManager.EXTRA_DEVICE_ADMIN, new ComponentName(this, DetoxAdmin.class))));
                case 4 -> confirm("Remove all paired devices from this phone?", "Forget", () -> {
                    if (service() != null) service().unpair();
                });
                case 5 -> stopBackgroundService();
            }
        }).setNegativeButton("Close", null).show();
    }

    // The guide to turning blocking on. From Android 13, an app installed from a file (not from
    // a store) can't have its accessibility switch turned on until "Allow restricted settings"
    // is chosen in its App info, and that menu item only appears after the grey switch has been
    // tapped once. So the guide goes step by step: each time the user comes back without
    // blocking on, it offers the next step (see onResume).
    private enum Step { NONE, ACCESSIBILITY, APP_INFO }
    private Step step = Step.NONE;
    private final Runnable guide = this::continueGuide;

    private void blockingPermission() {
        confirm("The accessibility service shows a blocking screen during Detox and Pomodoro breaks. It observes which app is in front and does not read screen text or typed content. Phone calls and emergency exit remain available. You can revoke permission in Android settings."
                + (installedFromFile() ? "\n\nNext: tap Digital Detox, then its switch. If the switch is grey, tap it anyway and come back here: you'll be shown how to unlock it." : "\n\nNext: tap Digital Detox, then turn its switch on."),
            "Open accessibility settings", this::openAccessibility);
    }

    private void openAccessibility() {
        step = Step.ACCESSIBILITY;
        startActivity(new Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS));
    }

    private void openAppInfo() {
        step = Step.APP_INFO;
        startActivity(new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS)
            .setData(android.net.Uri.fromParts("package", getPackageName(), null)));
    }

    /** Back from a step of the guide: offers the next one, unless blocking is on now */
    private void continueGuide() {
        Step from = step;
        step = Step.NONE;
        if (from == Step.NONE || BlockService.instance != null) return;
        if (from == Step.ACCESSIBILITY && installedFromFile()) {
            new AlertDialog.Builder(this).setTitle("Was the switch grey?")
                .setMessage("Android locks this setting for apps installed from a file.\n\n1. Open App info.\n2. Tap ⋮ in the top right corner.\n3. Tap \"Allow restricted settings\" and confirm.\n4. Come back here.\n\nThe menu item only appears after you've tapped the grey switch once.")
                .setPositiveButton("Open App info", (d, w) -> openAppInfo())
                .setNeutralButton("Try again", (d, w) -> openAccessibility())
                .setNegativeButton("Close", null).show();
        } else {
            new AlertDialog.Builder(this).setTitle("Turn blocking on")
                .setMessage("Open accessibility settings, tap Digital Detox and turn its switch on.")
                .setPositiveButton("Open accessibility settings", (d, w) -> openAccessibility())
                .setNegativeButton("Close", null).show();
        }
    }

    /** Installed from an APK file rather than a store: Android 13+ then restricts accessibility */
    private boolean installedFromFile() {
        if (Build.VERSION.SDK_INT < 33) return false;
        try {
            int source = getPackageManager().getInstallSourceInfo(getPackageName()).getPackageSource();
            return source != android.content.pm.PackageInstaller.PACKAGE_SOURCE_STORE;
        } catch (android.content.pm.PackageManager.NameNotFoundException e) {
            return true;
        }
    }

    private void stopBackgroundService() {
        DetoxService s = service();
        if (s == null || s.locked() || "pomodoro".equals(s.shared.optString("kind"))) {
            toast("Finish the current session first");
            return;
        }
        s.setSync(false);
        stopService(new Intent(this, DetoxService.class));
        finish();
    }

    private DetoxService service() { return DetoxService.instance; }

    private void publish(JSONObject session) {
        if (ready()) service().publish(session);
    }

    private boolean ready() {
        if (service() == null) toast("Service is starting. Try again shortly.");
        else if (BlockService.instance == null) toast("Enable the blocking permission first");
        else if (service().locked()) toast("A session is already running");
        else return true;
        return false;
    }

    private void confirm(String message, String action, Runnable run) {
        new AlertDialog.Builder(this).setTitle("Digital Detox").setMessage(message)
            .setPositiveButton(action, (d, w) -> run.run()).setNegativeButton("Cancel", null).show();
    }

    private void toast(String message) { Toast.makeText(this, message, Toast.LENGTH_LONG).show(); }

    private void select(int index) {
        selected = index;
        for (int i = 0; i < pages.size(); i++) {
            boolean active = i == index;
            pages.get(i).setVisibility(active ? View.VISIBLE : View.GONE);
            Button tab = tabs.get(i);
            tab.setSelected(active);
            tab.setTextColor(active ? WHITE : MUTED);
            tab.setBackground(ripple(active ? 0xff253951 : PANEL, 13, active ? BLUE : PANEL));
        }
        scroll.scrollTo(0, 0);
    }

    private final Runnable refresh = new Runnable() {
        @Override public void run() {
            boolean enabled = BlockService.instance != null;
            access.setText(enabled ? "●  Blocking is enabled" : "Blocking permission is required");
            access.setTextColor(enabled ? GREEN : MUTED);
            ((View) access.getTag()).setVisibility(enabled ? View.GONE : View.VISIBLE);
            updateUnlock();
            DetoxService s = service();
            if (s != null) refreshSession(s);
            handler.postDelayed(this, 1000);
        }
    };

    private void refreshSession(DetoxService s) {
        boolean running = "pomodoro".equals(s.shared.optString("kind"));
        boolean paused = running && s.shared.has("paused_at");
        Session.Phase current = Session.phase(s.shared, Session.now());
        Session.Plan plan = running ? Session.Plan.of(s.shared) : savedPlan();
        planSummary.setText(plan.work() / 60000 + " min focus  ·  " + plan.shortBreak() / 60000 + " min break  ·  "
            + plan.longBreak() / 60000 + " min long break" + (plan.total() > 0 ? "  ·  " + plan.total() + " rounds" : ""));
        changeTimes.setVisibility(running ? View.GONE : View.VISIBLE);
        int round = running ? current.round() : 1;
        countdown.setText(running ? Session.time(current.remaining()) : Session.time(plan.work()));
        phase.setText(paused ? "Paused" : current.locked() && running ? "Break in progress"
            : "Until your " + plan.breakAfter(round) / 60000 + "-minute break");
        rounds.setText("Round " + round + " of " + plan.rounds());
        startPomodoro.setVisibility(running ? View.GONE : View.VISIBLE);
        ((View) pause.getParent()).setVisibility(running ? View.VISIBLE : View.GONE);
        pause.setText(paused ? "Resume" : "Pause");
        pause.setEnabled(!s.locked());
        stop.setEnabled(!s.locked());
        sync.setText(s.syncEnabled ? "Sync enabled  ·  Turn off" : "Enable Sync");
        pairingCode.setText(s.code);
        pairingCode.setVisibility(s.code.isEmpty() ? View.GONE : View.VISIBLE);
        syncStatus.setText(s.status);
        syncStatus.setVisibility(s.status.isEmpty() ? View.GONE : View.VISIBLE);
        android.text.SpannableStringBuilder list = new android.text.SpannableStringBuilder();
        long now = SystemClock.elapsedRealtime();
        for (var entry : s.peers.entrySet()) {
            if (list.length() > 0) list.append("\n");
            Long seen = s.seen.get(entry.getKey());
            boolean online = seen != null && now - seen < 10000;
            int start = list.length();
            list.append("●");
            list.setSpan(new android.text.style.ForegroundColorSpan(online ? GREEN : MUTED),
                start, list.length(), android.text.Spanned.SPAN_EXCLUSIVE_EXCLUSIVE);
            list.append("  ").append(entry.getValue()).append(online ? "  ·  online" : "  ·  offline");
        }
        peers.setText(list.length() == 0 ? "No devices discovered yet" : list);
    }

    @Override protected void onSaveInstanceState(Bundle state) {
        super.onSaveInstanceState(state);
        state.putInt("tab", selected);
        state.putString("duration", minutes.getText().toString());
        // Android may close the app while its settings are open
        state.putString("guide", step.name());
    }

    @Override protected void onRestoreInstanceState(Bundle state) {
        super.onRestoreInstanceState(state);
        minutes.setText(state.getString("duration", "25"));
        step = Step.valueOf(state.getString("guide", Step.NONE.name()));
    }

    @Override protected void onResume() {
        super.onResume();
        handler.post(refresh);
        // A moment's wait: the service may connect just after the switch was turned on
        if (step != Step.NONE) handler.postDelayed(guide, 600);
    }

    @Override protected void onPause() {
        handler.removeCallbacks(refresh);
        handler.removeCallbacks(guide);
        super.onPause();
    }

    private LinearLayout page(String title, String subtitle) {
        LinearLayout page = column();
        page.setPadding(0, dp(22), 0, dp(22));
        TextView heading = label(title, 27, WHITE);
        heading.setTypeface(Typeface.DEFAULT, Typeface.BOLD);
        add(page, heading, 0);
        add(page, label(subtitle, 15, MUTED), 5);
        return page;
    }

    private LinearLayout card(LinearLayout parent) {
        LinearLayout card = column();
        card.setPadding(dp(16), dp(16), dp(16), dp(16));
        card.setBackground(shape(PANEL, 18, EDGE));
        add(parent, card, 18);
        return card;
    }

    private LinearLayout column() {
        LinearLayout layout = new LinearLayout(this);
        layout.setOrientation(LinearLayout.VERTICAL);
        return layout;
    }

    private LinearLayout row() {
        LinearLayout layout = new LinearLayout(this);
        layout.setGravity(Gravity.CENTER_VERTICAL);
        return layout;
    }

    private void add(LinearLayout parent, View view, int margin) {
        LinearLayout.LayoutParams params = new LinearLayout.LayoutParams(-1, -2);
        params.topMargin = dp(margin);
        parent.addView(view, params);
    }

    private EditText numberField(String text, String hint, int maxDigits) {
        EditText field = new EditText(this);
        field.setInputType(InputType.TYPE_CLASS_NUMBER);
        field.setFilters(new InputFilter[]{new InputFilter.LengthFilter(maxDigits)});
        field.setTextColor(WHITE);
        field.setHintTextColor(MUTED);
        field.setGravity(Gravity.CENTER);
        field.setSingleLine(true);
        field.setPadding(dp(10), dp(8), dp(10), dp(8));
        field.setBackground(shape(BG, 12, EDGE));
        field.setText(text);
        field.setHint(hint);
        return field;
    }

    private TextView label(String text, int size, int color) {
        TextView view = new TextView(this);
        view.setText(text);
        view.setTextSize(size);
        view.setTextColor(color);
        return view;
    }

    private Button button(String text, int color, Runnable action) {
        Button view = new Button(this);
        view.setAllCaps(false);
        view.setText(text);
        view.setTextSize(15);
        view.setTypeface(Typeface.DEFAULT, Typeface.BOLD);
        view.setTextColor(color == BLUE ? BG : Color.WHITE);
        view.setMinHeight(dp(48));
        view.setMinWidth(0);
        view.setPadding(dp(6), dp(8), dp(6), dp(8));
        view.setBackground(ripple(color, 13, color == PANEL || color == BG ? EDGE : color));
        view.setOnClickListener(v -> action.run());
        return view;
    }

    private GradientDrawable shape(int fill, int radius, int border) {
        GradientDrawable shape = new GradientDrawable();
        shape.setColor(fill);
        shape.setCornerRadius(dp(radius));
        shape.setStroke(dp(1), border);
        return shape;
    }

    private RippleDrawable ripple(int fill, int radius, int border) {
        return new RippleDrawable(ColorStateList.valueOf(0x33ffffff), shape(fill, radius, border), null);
    }

    private int dp(int value) {
        return Math.round(value * getResources().getDisplayMetrics().density);
    }
}
