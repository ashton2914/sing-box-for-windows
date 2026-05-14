# User Guide

## What This App Is

`sing-box for Windows` is a lightweight Windows launcher and manager for `sing-box`. It provides a native GUI for selecting cores, managing configs, starting and stopping the process, viewing logs, and configuring basic Windows integration.

The app is not a `sing-box` distribution. You must provide the `sing-box` core executable and your own configuration files.

## Installation

1. Download the release executable for your device.
2. Place the executable in a writable folder.
3. Run it.

No installer is required. In release builds, all runtime folders are created beside the executable.

## Runtime Folders

On first launch, the app creates these folders beside the executable:

```text
core/       sing-box core executables
config/     launcher-managed config entries and metadata
sing-box/   working directory for the running sing-box process
```

It also creates or updates:

```text
settings.json
```

The app may create this folder later:

```text
tools/      on-demand third-party helper downloads
```

`tools/` is not required for normal startup.

## Adding a sing-box Core

Put a Windows `sing-box` executable into `core/`.

Example:

```text
core/sing-box-windows-amd64.exe
```

Then choose it from the `Core` selector in Settings.

## Configs

Configs are managed from the `Configs` section.

Supported config sources are:

- local config files,
- remote config URLs.

Remote configs can be updated from the app. Auto-update behavior is controlled in Settings.

## Running

1. Select a config.
2. Select a core executable.
3. Press the main start button.

Logs appear in the main log panel. The app uses the `sing-box/` folder as the process working directory.

## Windows Startup

`Launch on Windows startup` registers normal per-user startup behavior.

`Silent startup` starts the app hidden when launched automatically.

## Administrator Mode

Some `sing-box` scenarios, especially TUN-related use cases, may require elevation. The app can register a per-user elevated scheduled task and hand off startup to it when administrator mode is enabled.

## AppContainer Loopback Utility

Windows AppContainer apps such as UWP apps, Microsoft Edge, and Store apps cannot always reach local loopback proxy addresses by default. The Settings page includes an `AppContainer loopback utility` action for this case.

Important notes:

- The utility is a third-party Telerik/Fiddler tool.
- This project does not bundle or redistribute it.
- On first use, the app downloads the official utility from Telerik's CDN into `tools/` and opens it.
- If the utility is already installed at `C:\Program Files (x86)\EnableLoopback\EnableLoopback.exe`, the app opens the installed copy.
- UAC may prompt because the utility performs system-level changes.

## About and License

The Settings page includes an About section with the current app version, copyright information, runtime-component disclaimer, and GPLv3 license text.
