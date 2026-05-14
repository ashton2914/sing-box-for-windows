# Release Guide

## Release Inputs

The release version comes from `Cargo.toml`.

The build script writes artifacts to:

```text
dist\v<version>\
```

For version `0.1.0`, the output directory is:

```text
dist\v0.1.0\
```

## Build Command

Run from the repository root:

```powershell
.\scripts\build-windows.ps1
```

The script builds:

```text
x86_64-pc-windows-msvc
aarch64-pc-windows-msvc
```

Expected artifacts:

```text
sing-box-for-windows-v0.1.0-windows-x86_64.exe
sing-box-for-windows-v0.1.0-windows-arm64.exe
SHA256SUMS.txt
```

## ARM64 Prerequisites

ARM64 builds may compile native dependencies such as `ring`. These require a C compiler.

Use one of these environments:

- Developer PowerShell for VS 2022 with C++ desktop workload and ARM64 tools installed.
- x64 Native Tools Command Prompt for VS 2022 with the same components.
- LLVM/clang with `clang.exe` available on `PATH`.

Recommended Visual Studio Build Tools components:

- Desktop development with C++
- Windows SDK
- MSVC x64/x86 build tools
- MSVC ARM64/ARM64EC build tools

## Release Checklist

1. Update `Cargo.toml` version.
2. Run `cargo check` to update `Cargo.lock`.
3. Run `cargo fmt`.
4. Run the release build script.
5. Confirm both executables exist in `dist\v<version>\`.
6. Confirm `SHA256SUMS.txt` contains hashes for both executables.
7. Launch the x86_64 executable on Windows.
8. Confirm first launch creates `core/`, `config/`, and `sing-box/` beside the executable.
9. Confirm About shows the expected version and license text.
10. Publish only the executable artifacts and checksums. Do not publish runtime folders.

## Distribution Notes

Users need only the launcher executable to start the app. Runtime folders are created automatically.

The release must not include:

- user configs,
- `settings.json`,
- `sing-box` core binaries,
- Telerik/Fiddler EnableLoopback binaries,
- generated runtime folders.

The release may include:

- launcher executable,
- checksums,
- source archive,
- license file.
