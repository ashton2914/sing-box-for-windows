# Release Notes

## v0.1.9 - 2026-05-27

### Fixed

- Fixed the v0.1.8 regression where the launcher still consumed several percent of CPU after the window was hidden to the tray, and where the first repaint after a tray-hide could briefly hang.
  - The previous fix relied on a cached "is the window visible" flag refreshed from inside `update()`, but `update()` stops being called once the window is hidden — so the cache got stuck on "visible" and the log-forwarder kept asking the UI thread to wake at ~20 Hz.
  - Replaced the cached flag with a live `IsWindowVisible` Win32 query against an `Arc<AtomicUsize>` HWND that the log forwarder reads each iteration. The forwarder now always reflects the current OS state instead of a snapshot from the last `update()` that ran.
  - Added a defensive early return inside `update()` itself: when the window is hidden, the entire `CentralPanel` / UI layout pass is skipped. This makes any residual wake (from leftover scheduled repaints or from winit's own scheduling for `SW_HIDE`'d windows) effectively free instead of costing a full ~10 ms layout pass.

### Notes

- The 50 ms (visible) / 2 s (hidden) repaint throttle from v0.1.8 is preserved, but it now actually engages reliably when the window is hidden.
- The earlier-known limitation that a right-click → Close on the **minimized** taskbar entry only fires once the window is restored is a separate Windows / winit behavior (paint requests are suppressed for iconic windows). It is unchanged by this fix.

## v0.1.8 - 2026-05-27

### Fixed

- Reduced idle CPU usage of the launcher itself while the window is hidden to the tray. The two dominant wake sources have been gated on actual window visibility:
  - The running-uptime label's 1 Hz self-refresh in the run card no longer schedules repaints when the window is hidden, so the UI thread is no longer woken once per second by an invisible widget.
  - The log forwarder no longer requests an immediate repaint per incoming log line. Bursts are now coalesced through `request_repaint_after` with a ~50 ms debounce while the window is visible, and a 2 s debounce while hidden, so chatty sing-box info-level output stops translating one-to-one into full layout passes.

### Notes

- A live `IsWindowVisible` query is published into a cross-thread atomic at the top of every frame, so the tray-hide / restore transition flips the throttling immediately on the next paint.
- The in-memory log ring buffer (`LOG_BACKLOG_CAP`) is unchanged; the slower hidden-window drain still empties the channel well within the buffer bound.
- No user-visible behavior or settings changed.

## v0.1.7 - 2026-05-19

### Added

- Added a **Core** chip in the log toolbar showing the running kernel's short version. Clicking the chip opens a modal with the full `<core> version` output, useful for confirming the active sing-box build tags when filing issues.
- Stale `selected_config` and `selected_core` settings are now cleared automatically when the underlying file or folder has been deleted, so the dropdowns no longer surface entries that resolve to nothing.

### Changed

- Redesigned the log toolbar so every label (Core, Log, Uptime, WebUI, Clear, Pause/Resume) shares a single typographic baseline. Interactive elements use a hidden-button chip style that only reveals chrome on hover, keeping the toolbar visually quiet until the pointer arrives.
- Clear and Pause/Resume are now always visible. The toolbar layout no longer shifts as logs arrive or are cleared.
- Reduced the default log panel height from 400 px to 200 px so the rest of the page has more vertical breathing room; the panel is still scrollable for older entries.

### Notes

- The Core chip caches `<core> version` output and refreshes only when the selected core changes or the binary on disk is replaced, with a 1-second debounce on the file-system probe so idle repaints stay free.
- Internal refactor: the log-toolbar chip painters were folded into a single `LogInlineChips` renderer. No behavioral change.

## v0.1.6 - 2026-05-18

### Fixed

- Fixed the "Destroy Working Dir" confirm dialog appearing to be missing its primary button while sing-box was still running. The dialog now switches to a dedicated "Stop sing-box first" prompt that clearly explains why the action is blocked and offers a single OK button, instead of showing a silently-disabled Destroy button.

## v0.1.5 - 2026-05-18

### Added

- Added settings controls for runtime inbound overrides, including mixed-in and TUN modes.
- Added runtime log override settings for disabled state, level, timestamps, and optional log-file output.
- Added a Theme selector with Dark, Light, and System modes.
- Added quick links to the sing-box GitHub repository and documentation from the Settings page.
- Added log-panel controls for pause/resume, clear, current log level, uptime, and Clash WebUI launch when available.

### Changed

- Restored the native Windows title bar and window frame so Snap, edge resize, maximize, and restore behavior follow Windows defaults.
- Refined Settings density, control alignment, input sizing, dividers, dropdown highlighting, and light/dark scrollbar colors.
- Reduced log status overhead by caching the parsed selected config and refreshing it only when the file changes.
- Deduplicated repeated log-level labels and settings external-link button handling.

### Fixed

- Fixed log text selection interfering with scroll interaction by rendering log entries as non-selectable labels.
- Fixed always-visible log scrollbars using overly bright handle colors.
- Fixed Settings labels not vertically aligning with text inputs, segmented controls, and compact row actions.
- Fixed several custom frameless-window interaction quirks by returning window movement and resizing to the OS frame.

## v0.1.4 - 2026-05-14

### Fixed

- Added backoff for failed automatic config updates so an expired remote config does not retry every 30 seconds while the source is unavailable.
- Settings and config metadata saves now serialize strictly and write through a temp/backup replacement flow instead of silently falling back to `{}`.
- Settings save failures are now reported back to the UI instead of being ignored by the background worker.
- Config replacement now reports backup/removal/rename failures with path-specific errors and attempts to restore the previous file when final replacement fails.
- Corrupt or unreadable `settings.json` and config `metadata.json` files now surface visible warnings instead of silently resetting settings or hiding config entries.
- Cached AppContainer loopback utility downloads are now reused only after a basic Windows PE header validation.

### Notes

- The auto-update interval is still based on each selected remote config's `last_updated` timestamp; the new retry delay only applies after failed automatic attempts.

## v0.1.3 - 2026-05-14

### Fixed

- Auto-update scheduling now uses the selected remote config's persisted `metadata.json` `last_updated` timestamp as the source of truth.
- Missing, malformed, or future `last_updated` values are treated conservatively and trigger an update attempt.
- Restarting the app no longer resets the auto-update interval through an in-memory timer, so the visible "Last updated" value matches the scheduling behavior.

### Notes

- Auto-update still applies only to the currently selected remote config.
- Local configs continue to use the manual Replace action.