param(
    [switch]$SkipTargetInstall
)

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Split-Path -Parent $ScriptDir
Set-Location $RepoRoot

function Assert-NativeCommandSucceeded {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Action
    )

    if ($LASTEXITCODE -ne 0) {
        throw "$Action failed with exit code $LASTEXITCODE"
    }
}

function Assert-TargetPrerequisites {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Triple
    )

    if ($Triple -ne "aarch64-pc-windows-msvc") {
        return
    }

    $HasCl = Get-Command cl.exe -ErrorAction SilentlyContinue
    $HasClang = Get-Command clang.exe -ErrorAction SilentlyContinue
    if (-not $HasCl -and -not $HasClang) {
        throw @"
ARM64 build requires a C compiler for native crates such as ring.

Open "Developer PowerShell for VS 2022" or "x64 Native Tools Command Prompt for VS 2022" with these Visual Studio Build Tools components installed:
  - Desktop development with C++
  - Windows SDK
  - MSVC x64/x86 build tools
  - MSVC ARM64/ARM64EC build tools

Alternatively install LLVM and make sure clang.exe is available on PATH.
"@
    }
}

$CargoToml = Get-Content -Raw -Path (Join-Path $RepoRoot "Cargo.toml")
if ($CargoToml -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
    throw "Unable to read package version from Cargo.toml"
}
$Version = $Matches[1]
$DistDir = Join-Path $RepoRoot "dist\v$Version"
New-Item -ItemType Directory -Force -Path $DistDir | Out-Null

$Targets = @(
    @{
        Triple = "x86_64-pc-windows-msvc"
        Label  = "windows-x86_64"
    },
    @{
        Triple = "aarch64-pc-windows-msvc"
        Label  = "windows-arm64"
    }
)

if (-not $SkipTargetInstall) {
    if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
        throw "rustup was not found. Install Rust via rustup, or rerun with -SkipTargetInstall after installing targets manually."
    }

    $InstalledTargets = @(rustup target list --installed)
    foreach ($Target in $Targets) {
        if ($InstalledTargets -notcontains $Target.Triple) {
            Write-Host "Installing Rust target $($Target.Triple)..."
            rustup target add $Target.Triple
            Assert-NativeCommandSucceeded "rustup target add $($Target.Triple)"
        }
    }
}

$ChecksumFile = Join-Path $DistDir "SHA256SUMS.txt"
if (Test-Path $ChecksumFile) {
    Remove-Item $ChecksumFile
}

foreach ($Target in $Targets) {
    Write-Host "Building $($Target.Triple)..."
    Assert-TargetPrerequisites -Triple $Target.Triple
    cargo build --release --locked --target $Target.Triple
    Assert-NativeCommandSucceeded "cargo build --target $($Target.Triple)"

    $SourceExe = Join-Path $RepoRoot "target\$($Target.Triple)\release\sing-box-for-windows.exe"
    if (-not (Test-Path $SourceExe)) {
        throw "Expected build output not found: $SourceExe"
    }

    $OutName = "sing-box-for-windows-v$Version-$($Target.Label).exe"
    $OutPath = Join-Path $DistDir $OutName
    Copy-Item -Force $SourceExe $OutPath

    $Hash = (Get-FileHash -Algorithm SHA256 -Path $OutPath).Hash.ToLowerInvariant()
    "$Hash  $OutName" | Add-Content -Encoding ascii -Path $ChecksumFile
}

Write-Host "Done. Release artifacts:"
Get-ChildItem $DistDir | Select-Object Name, Length, LastWriteTime