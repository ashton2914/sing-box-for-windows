# Architecture

## Goals

The app is designed as a small Windows-native launcher around `sing-box`.

Primary goals:

- single executable distribution for the launcher,
- runtime folders created automatically beside the executable,
- no bundled `sing-box` core or forbidden third-party binaries,
- simple, recoverable JSON-based state,
- responsive GUI with blocking work kept off the UI thread.

## Runtime Root

Path resolution is handled by `src/core/paths.rs`.

In debug builds, the runtime root is the current working directory.

In release builds, the runtime root is the executable directory.

The app creates:

```text
core/
config/
sing-box/
```

The app stores settings at:

```text
settings.json
```

## Main Components

```text
src/main.rs              native app bootstrap, icon setup, viewport config
src/app.rs               application state, background worker, event handling
src/ui.rs                page layout, cards, dialogs, user workflows
src/theme.rs             Material-style colors, widgets, modals, animations
src/chrome.rs            frameless title bar, resize handles, window outline
src/log_bus.rs           in-process log forwarding
src/config/              config entries, settings, remote updater
src/core/                Windows integration and process management
```

## Background Work

The UI must remain responsive. Blocking operations should run through the background command/event path in `src/app.rs`.

Examples of background work:

- remote config updates,
- process lifecycle operations that may block,
- on-demand helper downloads.

## Process Model

The launcher starts the selected `sing-box` executable from `core/` and uses `sing-box/` as the working directory.

The app should not assume ownership of the user's core binary. It should list executable files and let the user choose.

## UI Model

The UI uses `eframe`/`egui` with a custom Material-inspired dark theme.

Current UI principles:

- dense but readable desktop layout,
- no marketing-style landing surface,
- cards for functional sections only,
- compact controls with restrained transitions,
- custom frameless chrome with explicit resize handles.

## Third-Party Helper Policy

The AppContainer loopback utility is not bundled because redistribution is not allowed by its upstream license. The app may download it on demand from the upstream URL and open it for the user.

This policy should apply to future third-party runtime helpers as well: do not bundle a binary unless redistribution rights are clear and documented.
