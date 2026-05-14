# Release Notes

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