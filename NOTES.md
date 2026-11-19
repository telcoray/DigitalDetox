# Notes

Shared working notes for the agents on this project. Update at the end of each session; keep it short.

_Last updated: 2026-11-19_

## Current state

- Latest release: **v2.8.4** (2026-11-07), published on GitHub with installer, zip and the first
  signed Android APK; not sent to winget.
- v2.8.1 and v2.8.2 fixed the lock screen when monitors wake up or are reconnected during a lock:
  the lock re-covers every monitor (checked 4 times a second, logged as `[session] monitors changed`)
  and taskbars are looked up again each time, so ones Windows recreates are hidden and restored too.
  Tested by the user, monitor switched off and on during a detox: works.

- v2.8.3: the installer offers an optional "Start with Windows" checkbox (English/Italian), using the
  same per-user Run entry and `--background` as the tray menu; upgrades start from the current
  setting. Tested by the user.
- "Start on its own each day" (Pomodoro tab, off by default, saved in settings.json): on the first
  use of each day (day starts at 5:00; input in the last minute, past the Windows lock screen) a
  corner card counts down 30 s, then starts a Pomodoro on this PC only (user's choice: not sent
  to paired devices; a paired device's Idle doesn't stop it). User tested the card and the
  automatic start; not yet checked that laptop and phone stay untouched.
- New Schedule tab (tabs are now 0 Detox, 1 Pomodoro, 2 Schedule, 3 Sync), not yet released:
  - set times (schedule.rs): up to 4 detoxes, e.g. 13:00 for 30 min, each on chosen weekdays
    (bitmask in settings.json, Monday = bit 0; older entries default to every day); a corner card counts
    down the last minute ("Start now" / "Skip"), then locks. This PC only, skipped during a
    Pomodoro or while Windows is locked, not fired after sleeping through it (> 2 min late).
  - "Break after 90 minutes" (break_reminder.rs, off by default): after 90 min of use (5 min
    without input, sleep, a lock or a Pomodoro reset it) a card offers a 10-minute break
    ("Take a break" / "Later" = again in 15 min). This PC only.
  - User's choices (2026-11-19): 90 → 10 min, 1-minute warning then lock, this PC only, own tab.
  - Only one corner card at a time (`card_shown` in main.rs); a set time pushes the others away.
  - Not yet tested by the user.
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

- Package `telcoray.DigitalDetox`. The last version on winget is **2.6.1**.
- From 2.7.0 on, the user chose not to send every release to winget, to avoid flooding the
  reviewers. The `winget` workflow is disabled in GitHub Actions; releases are published without it.
- To catch up: re-enable the workflow, or run it by hand with the latest tag (Actions → winget →
  Run workflow). It needs the `WINGET_TOKEN` secret, already set. Only when the user asks.

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
