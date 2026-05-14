# Development Guide

## Requirements

- Windows 10 or later.
- Rust stable toolchain.
- PowerShell 7 or Windows PowerShell.
- Visual Studio Build Tools for release builds that need MSVC native dependencies.

For ARM64 release builds, install C++ ARM64 tools or provide `clang.exe` on `PATH`.

## Common Commands

```powershell
cargo check
cargo fmt
cargo run
cargo build --release
```

Build release artifacts:

```powershell
.\scripts\build-windows.ps1
```

## Coding Principles

- Keep the launcher narrowly scoped. Do not turn it into a bundled proxy distribution.
- Keep blocking I/O off the UI thread.
- Prefer small, explicit state transitions over broad refactors.
- Preserve single-exe distribution for the launcher.
- Keep runtime data out of Git.
- Treat third-party binary redistribution conservatively.

## UI Guidelines

- Use `src/theme.rs` for shared colors, buttons, modals, switches, and animations.
- Keep settings controls compact and aligned.
- Avoid high-emphasis buttons for secondary actions.
- Keep motion short and functional. Transitions should clarify state changes, not decorate them.
- Preserve minimum window size when changing modal or settings layouts.

## Filesystem Guidelines

Runtime data belongs beside the executable in release builds and is ignored by Git:

```text
config/
core/
sing-box/
tools/
settings.json
```

Do not commit generated release artifacts from:

```text
dist/
target/
```

## Versioning

The package version in `Cargo.toml` is the source of truth for release artifact names. After changing it, run `cargo check` or another Cargo command to update `Cargo.lock`.

## Adding Features

Before adding a feature, check whether it belongs in this launcher.

Good fit:

- local process management,
- config lifecycle improvements,
- Windows integration around startup, tray, elevation, and loopback,
- safer release packaging,
- UI clarity and reliability.

Poor fit:

- bundling proxy cores,
- shipping third-party binaries without clear redistribution rights,
- adding cloud account systems,
- expanding into a general network diagnostics suite.

## Testing Expectations

At minimum, run:

```powershell
cargo fmt
cargo check
```

For release changes, also run:

```powershell
.\scripts\build-windows.ps1
```

If ARM64 prerequisites are missing, x86_64 may still build successfully, but the full script should be run from a properly configured Developer PowerShell before publishing a dual-architecture release.
