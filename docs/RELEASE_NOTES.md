# Release Notes

## v0.1.3 - 2026-05-14

### Fixed

- Auto-update scheduling now uses the selected remote config's persisted `metadata.json` `last_updated` timestamp as the source of truth.
- Missing, malformed, or future `last_updated` values are treated conservatively and trigger an update attempt.
- Restarting the app no longer resets the auto-update interval through an in-memory timer, so the visible "Last updated" value matches the scheduling behavior.

### Notes

- Auto-update still applies only to the currently selected remote config.
- Local configs continue to use the manual Replace action.