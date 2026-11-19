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
    private Button pause, stop, startPomodoro, sync;
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
                () -> publish(DetoxService.obj("kind", "pomodoro", "start", Session.now())));
        });
        add(page, startPomodoro, 14);
        LinearLayout actions = row();
        pause = button("Pause", PANEL, this::togglePause);
        stop = button("Stop", PANEL, () -> {
            DetoxService s = service();
            if (s != null && !s.locked() && "pomodoro".equals(s.shared.optString("kind"))) {
                s.publish(DetoxService.obj("kind", "idle"));
            }
        });
        actions.addView(pause, new LinearLayout.LayoutParams(0, -2, 1));
        LinearLayout.LayoutParams right = new LinearLayout.LayoutParams(0, -2, 1);
        right.leftMargin = dp(8);
        actions.addView(stop, right);
        add(page, actions, 12);
        return page;
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
        s.publish(updated);
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

    private void blockingPermission() {
        confirm("The accessibility service shows a blocking screen during Detox and Pomodoro breaks. It observes which app is in front and does not read screen text or typed content. Phone calls and emergency exit remain available. You can revoke permission in Android settings.",
            "Open accessibility settings", () -> startActivity(new Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS)));
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
        Session.Plan plan = Session.Plan.of(running ? s.shared : new JSONObject());
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
    }

    @Override protected void onRestoreInstanceState(Bundle state) {
        super.onRestoreInstanceState(state);
        minutes.setText(state.getString("duration", "25"));
    }

    @Override protected void onResume() {
        super.onResume();
        handler.post(refresh);
    }

    @Override protected void onPause() {
        handler.removeCallbacks(refresh);
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
