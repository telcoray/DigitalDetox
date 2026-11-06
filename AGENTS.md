# Digital Detox: instructions for AI agents

These rules are shared by every agent working on this repository (Claude Code, Codex, …).
`CLAUDE.md` only imports this file, so edit the rules here.

## Before and after you work

- **At the start**, read `NOTES.md` (current state, decisions, open work) and `git log --oneline -15`.
- **At the end**, update `NOTES.md` if anything there changed: state, a decision and why, a new
  open item, a known issue. Keep it short and remove what is no longer true. Stale notes are worse
  than none.
- Only one agent works on `main` at a time. To work in parallel, each agent uses its own branch
  (git worktree) and the changes are merged afterwards. Never edit files another agent is working on.

## Talking to the user

- Reply in **Italian**. Code, comments, commit messages, README and release notes are in **English**.
- The user tests the app by hand on real hardware: a desktop PC with two monitors (one horizontal,
  one vertical) and a laptop, paired with Sync. When you change anything that depends on screens,
  power, the lock or the network, say exactly what to try and what to look for.

## Project

A Windows tray app in Rust with a [Slint](https://slint.dev/) UI that locks the screen for a set
time (detox), runs Pomodoro cycles, and syncs both across paired devices on the local network.
The layout is described in `README.md` → "Project layout".

## Commands

```bash
cargo build                                   # debug build
cargo build --release --locked                # what CI and releases use
cargo test --release --locked
cargo clippy --release --locked -- -D warnings
```

All three CI checks (build, test, clippy with no warnings) must pass before a commit.

The app's log is `%LOCALAPPDATA%\DigitalDetox\digital-detox.log`, rewritten at every start; sync
pairing lives in `%LOCALAPPDATA%\DigitalDetox\sync.json`. Read the log when the user reports a bug.

## Rules that are easy to break

- **Never start a detox, a Pomodoro or a sync yourself** unless the user asks: it locks the user's
  screens (and, with Sync, the laptop too) and the agent can't unlock them.
- Only one copy of the app runs at a time. To try a build, the installed copy must be closed first.
- Keep `default-features = false` on Slint and **don't add the `accessibility` feature**: its
  AccessKit layer crashed the app when monitors woke up during a lock.
- Keep the software renderer as the default (about 5 MB of memory instead of ~145 MB with Skia).
- Keep `+crt-static` in `.cargo/config.toml`: without it the app fails to start on a fresh Windows.
- Never change the `AppId` in `installer/DigitalDetox.iss`: it's how new installers upgrade old ones.
- `release/` and `target/` aren't committed.
- `ROADMAP.md` is private (listed in `.git/info/exclude`): never commit or publish it.

## Git

- Commit messages: one short sentence in plain English describing the change for the user, no
  prefixes like `feat:`. E.g. "Cover the monitors again when they change during a lock".
- A version bump is its own commit named `Version X.Y.Z`, changing only `Cargo.toml` and `Cargo.lock`.
- End commit messages with your own `Co-Authored-By:` line.
- **Ask before** pushing, tagging, creating or publishing a release, or anything that touches winget.
  Committing locally is fine when the user asked for the change and the checks pass.

## Releases

1. Commit `Version X.Y.Z`, push `main`.
2. Push the tag `vX.Y.Z`: the Release workflow builds the zip and the installer and creates a
   **draft** release (it fails if the tag doesn't match `Cargo.toml`).
3. Write the release notes (English, short, what changed for the user) and publish only when the
   user says so.

The `winget` workflow is currently **disabled** (see `NOTES.md`), so publishing doesn't update winget.
