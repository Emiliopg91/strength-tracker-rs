# Strength Tracker

[![Ask DeepWiki](https://deepwiki.com/badge.svg)](https://deepwiki.com/Emiliopg91/strength-tracker-rs)

**Everything Garmin Connect doesn't do for your strength training — on Linux, offline, with your data in a local file.**

Your Garmin watch records every set you lift, but Garmin Connect does little with it: exercises get misdetected, sets are tedious to fix, and nothing tells you when it's time to add weight. Strength Tracker is a Linux desktop app that takes over from there. Plug in your watch, and your sessions are imported, analyzed and turned into your next workout — which goes straight back to the watch.

No account required, no cloud, no Garmin Express (which doesn't exist on Linux anyway). Just a USB cable and a SQLite database you own — with optional Garmin Connect import for when the cable isn't at hand.

## Screenshots

| ![Screenshot 1](resources/screenshots/screenshot1.png) | ![Screenshot 2](resources/screenshots/screenshot2.png) |
| :----------------------------------------------------: | :----------------------------------------------------: |
| ![Screenshot 3](resources/screenshots/screenshot3.png) | ![Screenshot 4](resources/screenshots/screenshot4.png) |
| ![Screenshot 5](resources/screenshots/screenshot5.png) |

## The training loop

1. **Plan** — Build strength workouts in the app and send them to your watch.
2. **Train** — Follow the workout on the watch as usual.
3. **Sync** — Plug the watch in. The app can launch itself on connect and imports new activities automatically.
4. **Progress** — Strength Tracker checks each exercise against its rep range and suggests your next target. Apply the suggestion and the affected workouts are updated and queued for your watch, ready for the next session.

## Progression, done for you

- **Double progression per exercise** — Set a rep range such as `8-10-12` and a weight increment. Your weakest set sets the pace: once every set reaches the top of the range, the app suggests adding the increment and starting again from the bottom; otherwise it moves you to the next rep step.
- **Only when it's earned** — No suggestion while any set is below the range or sets were done at mixed weights. Progression can be turned off per exercise.
- **Unilateral aware** — Rep ranges count per side for single-arm or single-leg exercises.
- **One click to apply** — Accepted suggestions update your workouts and are pushed to the watch on its next connection.

## Strength analysis

- **Session review and editing** — Fix exercises, sets, reps and weights, add notes, and hide the exercises chart on sessions where it adds nothing.
- **Muscles worked** — Every catalog exercise lists its primary and secondary muscles, and exercises, sessions and workouts show them on a front and back body map.
- **Personal records** — 1RM estimation per exercise, with a desktop notification when you set a new PR.
- **Exercise history** — Full per-session set history and volume chart for every exercise.
- **Exercise notes** — Keep cues and setup notes per exercise; workout steps without their own notes inherit them, both on the watch and in printed workouts.
- **Session comparison** — Compare sessions and their exercises side by side.
- **Training load** — Daily load heatmap and workload chart on the home page.
- **Progression report** — Printable report for any date range, covering sessions, exercises, body composition and notes.

## Beyond the gym

- **Cardio activities** — Heart-rate chart with time-in-zone breakdown, speed and laps.
- **GPS routes** — Interactive map with a speed-colored track, satellite/street toggle, reverse-geocoded naming and GPX export with elevation.
- **Body measurements** — Log them manually or import them from Garmin Connect, then compare and chart them over time with 7-day averages.

## Device sync

- **USB (MTP) sync** — Auto-detects Garmin devices and downloads new activities.
- **Workout upload queue** — Workouts sent or changed while the watch is disconnected are pushed automatically on its next connection.
- **Workout import** — Import workout steps back from the device.
- **Print workouts** — Print a workout or save it as PDF to take to the gym.
- **Manual `.FIT` import** — Import activity files from disk.
- **Garmin Connect import** — Optional, on-demand download of new activities and weigh-ins (including body composition from smart scales) since the last sync. Sign in once through a `zenity` dialog; the session is kept in the system keyring and refreshed automatically.

## Your data, your machine

- **Local SQLite database** — Versioned migrations, applied automatically.
- **Credentials stay in your keyring** — Garmin Connect session tokens live in the system keyring (Secret Service), not in the database, and your password is never saved.
- **JSON export** — Full database export for backup or your own analysis.
- **Cloud backup, if you want it** — One-click upload to OneDrive or Dropbox via `rclone`.

## Desktop integration

- **System tray** — Start minimized and keep running in the background.
- **Launch on boot or on device connect.**
- **Native notifications** — Localized alerts for imports, PRs and updates.
- **English and Spanish**, metric or imperial units, all applied live.
- **Update check** — Notifies you when a new release is available.

## Installation

### Arch Linux (via `AUR helper`/`PKGBUILD`)

Install the AUR `strength-tracker-rs` package to get latest stable version of the application and every external dependency.

### From source

Requirements: [Rust](https://www.rust-lang.org/tools/install), [pnpm](https://pnpm.io/), and the [Tauri prerequisites](https://tauri.app/start/prerequisites/) for your platform.

```bash
git clone https://github.com/Emiliopg91/strength-tracker-rs.git
cd strength-tracker-rs
pnpm install
make build       # or: pnpm tauri build
```

## Development

Built with [Tauri](https://tauri.app/): a Rust backend with a React + TypeScript frontend.

```bash
pnpm install
make run          # runs the app in dev mode
make test         # frontend and backend tests
make lint         # eslint, tsc and clippy
```

Development builds use [`mold`](https://github.com/rui314/mold) as the linker and [`sccache`](https://github.com/mozilla/sccache) for compilation caching to speed up iteration; install both for the best experience (or adjust the `Makefile` if you don't have them).

## License

Distributed under the GPL-2.0 license, as declared in the project's `PKGBUILD`.
