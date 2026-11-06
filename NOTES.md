# Notes

Shared working notes for the agents on this project. Update at the end of each session; keep it short.

_Last updated: 2026-11-06_

## Current state

- Latest release: **v2.8.2** (2026-11-02), published on GitHub. Version **2.8.3** is committed and pushed (main and tag v2.8.3); GitHub workflows generate the draft release. Publication is pending.
- v2.8.1 and v2.8.2 fixed the lock screen when monitors wake up or are reconnected during a lock:
  the lock re-covers every monitor (checked 4 times a second, logged as `[session] monitors changed`)
  and taskbars are looked up again each time, so ones Windows recreates are hidden and restored too.
  Tested by the user, monitor switched off and on during a detox: works.

- v2.8.3: installer now offers an optional "Start with Windows" checkbox (English/Italian), using the same per-user Run entry and `--background` as the tray menu. Upgrades read the current startup setting. Fixed the initial preview startup crash: initialize the startup task at wpSelectTasks, after {app} becomes available. Compilation and a silent runtime regression check passed (stopped in PrepareToInstall before installation). Interactive install/upgrade/sign-in checks remain.
- Installer Italian tasks page now says "Opzioni aggiuntive" with the user-requested plain-language instruction; release 2.8.3 build, all 12 tests, Clippy and installer compilation passed locally.
- Tray visibility remains a Windows user preference: drag the icon out of the hidden-icons menu to keep it beside the clock.

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
