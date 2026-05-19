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

function Get-VsWherePath {
    $candidates = @(
        (Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"),
        (Join-Path $env:ProgramFiles "Microsoft Visual Studio\Installer\vswhere.exe")
    )
    foreach ($p in $candidates) {
        if ($p -and (Test-Path -LiteralPath $p)) {
            return $p
        }
    }
    return $null
}

function Get-VsInstallPathWithComponent {
    param(
        [Parameter(Mandatory = $true)]
        [string]$ComponentId
    )

    $vswhere = Get-VsWherePath
    if (-not $vswhere) {
        return $null
    }

    $install = & $vswhere -latest -products * -requires $ComponentId -property installationPath 2>$null
    if ([string]::IsNullOrWhiteSpace($install)) {
        return $null
    }
    return ($install | Select-Object -First 1).Trim()
}

function Ensure-ClangOnPath {
    # If clang is already on PATH, nothing to do.
    if (Get-Command clang.exe -ErrorAction SilentlyContinue) {
        return $true
    }

    # Try to find the LLVM/Clang component bundled with Visual Studio and
    # add its bin directory to PATH for the rest of this build session.
    $vsInstall = Get-VsInstallPathWithComponent -ComponentId "Microsoft.VisualStudio.Component.VC.Llvm.Clang"
    if ($vsInstall) {
        $llvmBin = Join-Path $vsInstall "VC\Tools\Llvm\bin"
        $clangExe = Join-Path $llvmBin "clang.exe"
        if (Test-Path -LiteralPath $clangExe) {
            $env:PATH = "$llvmBin;$env:PATH"
            Write-Host "Using clang from Visual Studio: $clangExe"
            return $true
        }
    }

    # Fallback: a standalone LLVM install in the usual location.
    $standaloneLlvm = Join-Path $env:ProgramFiles "LLVM\bin\clang.exe"
    if (Test-Path -LiteralPath $standaloneLlvm) {
        $env:PATH = (Split-Path -Parent $standaloneLlvm) + ";$env:PATH"
        Write-Host "Using clang from standalone LLVM: $standaloneLlvm"
        return $true
    }

    return $false
}

function Assert-TargetPrerequisites {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Triple
    )

    if ($Triple -ne "aarch64-pc-windows-msvc") {
        return
    }

    # `ring` (and a few other native crates) require clang to assemble ARM64
    # sources on Windows — cl.exe is not sufficient even with the ARM64 MSVC
    # tools installed. Linking and libc still come from MSVC.

    $haveClang = Ensure-ClangOnPath
    $haveArm64Msvc = $null -ne (Get-VsInstallPathWithComponent -ComponentId "Microsoft.VisualStudio.Component.VC.Tools.ARM64")
    $haveAnyVs = $null -ne (Get-VsWherePath)

    if ($haveClang -and $haveArm64Msvc) {
        return
    }

    if (-not $haveAnyVs) {
        throw @"
ARM64 build requires both clang (to compile ring's ARM64 assembly) and the
MSVC ARM64 toolchain (for linking and the C runtime), but no Visual Studio
installation was detected.

Install Visual Studio Build Tools 2022 with these components:
  - Desktop development with C++
  - Windows SDK
  - MSVC v143 - VS 2022 C++ x64/x86 build tools
  - MSVC v143 - VS 2022 C++ ARM64/ARM64EC build tools
  - C++ Clang Compiler for Windows  (or install LLVM separately)

Alternatively install LLVM standalone and add clang.exe to PATH.
"@
    }

    $missing = @()
    if (-not $haveArm64Msvc) {
        $missing += "  - MSVC v143 - VS 2022 C++ ARM64/ARM64EC build tools  (component id: Microsoft.VisualStudio.Component.VC.Tools.ARM64)"
    }
    if (-not $haveClang) {
        $missing += "  - C++ Clang Compiler for Windows  (component id: Microsoft.VisualStudio.Component.VC.Llvm.Clang)`n    Or install LLVM separately so that clang.exe is on PATH."
    }

    $list = ($missing -join "`n")
    throw @"
ARM64 build is missing required Visual Studio component(s):

$list

Open the Visual Studio Installer, click "Modify" on your VS 2022 / Build Tools
install, and add the components above on the "Individual components" tab.
"@
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