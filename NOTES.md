# Notes

Shared working notes for the agents on this project. Update at the end of each session; keep it short.

_Last updated: 2026-11-21_

## Current state

- Latest release: **v2.9.0** (2026-11-20), published on GitHub with installer, zip and signed APK,
  and sent to winget (PR microsoft/winget-pkgs#484485, waiting for review). It contains everything
  below. The user's phone and tablet still run the debug APK: moving them to the signed one needs
  an uninstall and a new pairing.
- v2.8.1 and v2.8.2 fixed the lock screen when monitors wake up or are reconnected during a lock:
  the lock re-covers every monitor (checked 4 times a second, logged as `[session] monitors changed`)
  and taskbars are looked up again each time, so ones Windows recreates are hidden and restored too.
  Tested by the user, monitor switched off and on during a detox: works.

- v2.8.3: the installer offers an optional "Start with Windows" checkbox (English/Italian), using the
  same per-user Run entry and `--background` as the tray menu; upgrades start from the current
  setting. Tested by the user.
- "Start on its own each day" (Pomodoro tab, off by default, saved in settings.json): on the first
  use of each day (day starts at 5:00; input in the last minute, past the Windows lock screen) a
  corner card counts down 30 s, then starts a Pomodoro. User tested the card and the automatic
  start. After 2.9.0 (user's choice, 2026-11-20) it's shared like any Pomodoro: the PC's or
  laptop's daily Pomodoro starts on every paired device, while the phone's stays on the phone.
  So every Pomodoro on Windows is shared now (`PomodoroRun::shared` was removed). A Pomodoro left
  paused since an earlier day (e.g. paused on the phone at 1:25) no longer blocks it: today's
  replaces it. Both not yet tested.
- Android: after "Set up blocking", a step-by-step guide for apps installed from a file (Android
  13+ "restricted settings": tap the grey switch, App info → ⋮ → Allow restricted settings).
  After 2.9.0; not yet tested. A signed APK without a release: run the Release workflow by hand,
  download the `android-files` artifact.
- Android Pomodoro tab (after 2.9.0, not yet tested): "Change times" (same plan and limits as
  Windows, saved in the "pomodoro" prefs, sent with a Pomodoro started on the phone) and "Start
  on its own each day" with a "From" time (user's idea, default 08:00: phone use in the morning
  before school/work doesn't count). The first unlock after that time shows a notification
  counting down 30 s ("Start now" / "Not today"), then a Pomodoro starts on the phone only
  (`DetoxService.local`: kept out of sync, like Windows' unshared daily Pomodoro). Windows keeps
  its fixed 5:00 day start: the user doesn't want the "From" time on the PC.
  User tested: custom times from the phone are followed by the PC; the daily start works (the
  notification wasn't noticed: Honor shows new channels without a banner).
- Android time pickers (Schedule's "Add a time", Pomodoro's "From") are two vertical wheels
  (NumberPicker, hours and minutes) inside the dialog, at the user's request (2026-11-21).
- Testing Android on the PC: the SDK has the AVD `Medium_Phone_API_36.1` (x86_64; it runs the
  arm64 native library through ARM translation). Start it with
  `C:\Android\emulator\emulator.exe -avd Medium_Phone_API_36.1`, then
  `adb install -r app\build\outputs\apk\debug\app-debug.apk`. Its Sync can't reach the LAN.
- Android, after that (not yet tested): days of the week for the daily Pomodoro, and a Schedule
  tab like Windows' (up to 4 set times with days, notification a minute before with Start now /
  Skip, then a detox on this phone only, `publishLocal`; skipped during a Pomodoro).
- New Schedule tab (tabs are now 0 Detox, 1 Pomodoro, 2 Schedule, 3 Sync), since 2.9.0:
  - set times (schedule.rs): up to 4 detoxes, e.g. 13:00 for 30 min, each on chosen weekdays
    (bitmask in settings.json, Monday = bit 0; older entries default to every day); a corner card counts
    down the last minute ("Start now" / "Skip"), then locks. This PC only, skipped during a
    Pomodoro or while Windows is locked, not fired after sleeping through it (> 2 min late).
  - "Break after 90 minutes" (break_reminder.rs, off by default): after 90 min of use (5 min
    without input, sleep, a lock or a Pomodoro reset it) a card offers a 10-minute break
    ("Take a break" / "Later" = again in 15 min). This PC only.
  - User's choices (2026-11-19): 90 → 10 min, 1-minute warning then lock, this PC only, own tab.
  - Only one corner card at a time (`card_shown` in main.rs); a set time pushes the others away.
  - Tested by the user: set times (with Start now, Skip, Edit, days) and the card's hide arrow.
- Pomodoro plan ("Change times" in the Pomodoro tab, plan_editor.rs; saved as `pomodoro` in
  settings.json), since 2.9.0: focus / short / long break minutes, rounds before the long
  break, and total rounds (0 = until stopped; after the last round's break it stops and publishes
  Idle). User's choice: the plan travels with the shared Pomodoro (`plan` field in
  `Shared::Pomodoro`, serde default = 25/5/15 × 4, so older versions interoperate but ignore it).
  Android reads it in Session.java, keeps it on pause/resume, goes idle when done; it can't edit
  a plan yet (always starts 25/5/15 × 4). User tested a 1/1/2-minute, 3-round plan on the PC (it
  stops by itself) and "Back to the usual"; not yet tested on the phone or with a paired PC.
- Sync with four devices (PC, laptop, phone, tablet, using the debug APK on both Android
  devices) tested by the user: a 1-minute lock reached all four. A reinstalled device comes back
  with a new id. The Sync list now drops entries with the same name that aren't online
  (`Config::remember_peer`), so the old one doesn't stay "offline" forever. User confirmed this.
  A custom Pomodoro plan started on the PC was followed by all four.
- Clock correction (src/sync/clock.rs): the tablet's countdown was 4–5 s ahead even with
  automatic time on. Every State message now carries `sent_at` (the sender's raw clock, optional
  so older versions interoperate); each device follows the median of all clocks, its own
  included. Windows uses it through `time_format::now_ms`, Android through `Session.now()`
  (offset returned by the native `poll`). User tested on all four: almost no difference left.
- Pomodoro round dots are filled circles (red up to the current round, grey after): the
  software renderer draws thin round borders with an uneven thickness, so no rings anywhere.
- "Start on its own each day" switch is now on the Pomodoro tab's first screen and in the tray menu.
- Tray visibility remains a Windows user preference: drag the icon out of the hidden-icons menu to keep it beside the clock.

## Android preview

- On `main` (merged from `android-preview`): Android 11+ ARM64 app in `android/`, targeted at HONOR 400 Lite / Android 16.
- Native Rust adapter reuses the existing Windows Sync worker/protocol. Native Android UI,
  accessibility overlay, optional administrator uninstall friction, Detox/Pomodoro, Keystore pairing.
- 4 Java schedule tests and 3 native protocol tests pass. APK builds, signature verifies, Android
  Lint has no errors; native library uses 16 KB alignment. User verified a one-minute Detox and automatic unlock on the HONOR 400 Lite (Android 16).
  The phone appears online in Windows Sync after pairing. Starting Detox from either device locks both.
  User also verified synchronized Pomodoro from the phone: both devices lock after 25 minutes,
  unlock after the 5-minute break and start the next focus round.
- APK 0.1.0-alpha2 brings the Windows-style Detox, Pomodoro and Sync tabs to Android, with duration presets, a focus timer and a separate Settings menu. Android build, unit tests and Lint pass. User reviewed all three tabs on the HONOR and confirmed the PC remains paired and online after upgrading.
- F-Droid preparation remains open.
- From v2.8.4 the APK ships in every GitHub release, with the Windows version number
  (user's choice), signed with a release key the user creates and stores as repository secrets
  (see android/README.md). The secrets are set; a manual Release run on 2026-11-07 built and
  signed the APK (certificate SHA-256 bb95061f…dd00). Switching the phone from the debug build to
  the signed one needs one uninstall and a new pairing.

## winget

- Package `telcoray.DigitalDetox`. Last version sent: **2.9.0** (2026-11-20, PR #484485 on
  microsoft/winget-pkgs, waiting for review); before that 2.6.1.
- From 2.7.0 on, the user chose not to send every release to winget, to avoid flooding the
  reviewers. The `winget` workflow is disabled in GitHub Actions; releases are published without it.
- To catch up: re-enable the workflow, or run it by hand with the latest tag (Actions → winget →
  Run workflow). It needs the `WINGET_TOKEN` secret, already set. Only when the user asks.
- For 2.9.0 it was enabled, run by hand and disabled again. If it fails with "The forked
  repository could not be synced", the user clicks Sync fork on github.com/telcoray/winget-pkgs
  (the local gh token lacks the `workflow` scope that syncing needs), then it's run again.

## Decisions

- `AGENTS.md`, `CLAUDE.md` and `NOTES.md` are committed, so every agent (and every worktree) sees
  the same rules and state. `ROADMAP.md` stays local only.

- v2 is a rewrite in Rust + Slint of the v1 C#/WPF app (still at tag `v1.0.1`).
- The breathing guide follows Buteyko's reduced breathing (2 s in, 3 s out, 4 s pause) and is
  deliberately left unexplained in the app. An explanation may be added later.
- Pomodoro red: `#ED1D25`, chosen by the user ("for now").
- Sync works on the local network only, no account or server; pairing with a 6-digit code (SPAKE2),
  messages signed with HMAC.

## Open / ideas

- Code signing with SignPath: looked into, set aside for now. Until then SmartScreen warns on
  first run.
- Send the latest version to winget (see above).

## Known limitations

- Ctrl+Alt+Del can't be blocked by any app (documented in the README).
