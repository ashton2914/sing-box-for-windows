# sing-box for Windows

A lightweight Windows launcher and manager for running `sing-box` with a small native GUI.

This project provides the launcher only. It does not bundle or redistribute the `sing-box` core binary, user proxy configurations, or third-party runtime utilities.

## Features

- Single executable launcher for Windows.
- Automatic creation of runtime folders beside the executable.
- Local and remote configuration management.
- Optional launch on Windows startup.
- Optional elevated startup flow for TUN-related scenarios.
- Built-in log view and tray integration.
- On-demand AppContainer loopback utility launcher for UWP, Edge, and Store app proxy access.

## Quick Start

1. Download `sing-box-for-windows-v<version>-windows-x86_64.exe` from a release.
2. Place it in any writable folder.
3. Run the executable once. The app creates the required runtime folders automatically.
4. Put your `sing-box` Windows core executable into `core/`.
5. Add or import a config in the app.
6. Select a config and start the service.

Created runtime layout:

```text
.
|-- sing-box-for-windows.exe
|-- core/
|-- config/
|-- sing-box/
|-- settings.json
```

The optional `tools/` folder is created only when the AppContainer loopback utility is downloaded on demand.

## Build

```powershell
.\scripts\build-windows.ps1
```

Artifacts are written to:

```text
dist\v<version>\
```

The script builds Windows x86_64 and ARM64 targets. ARM64 builds require a C compiler environment such as Visual Studio Build Tools with ARM64 C++ tools, or LLVM/clang on `PATH`.

## Documentation

- [User Guide](docs/USER_GUIDE.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Development Guide](docs/DEVELOPMENT.md)
- [Release Guide](docs/RELEASE.md)

## Project Direction

The project is intentionally scoped as a clean Windows launcher, not a bundled proxy distribution. Future development should prioritize:

- predictable single-exe deployment,
- explicit user control over runtime components,
- legally conservative handling of third-party binaries,
- stable, quiet desktop UI behavior,
- small release artifacts and simple recovery from runtime file corruption.

## License

Copyright (C) 2026 ashton2914.

Licensed under the GNU General Public License v3.0 or later. See [LICENSE](LICENSE).
