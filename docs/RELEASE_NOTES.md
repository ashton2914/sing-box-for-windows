# Release Notes

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