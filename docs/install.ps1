# browsync installer for Windows
# Usage: irm https://ishaileshpant.github.io/browsync/install.ps1 | iex
$ErrorActionPreference = "Stop"

$Repo = "ishaileshpant/browsync"
$InstallDir = "$env:LOCALAPPDATA\browsync\bin"

function Write-Info($msg) { Write-Host "[browsync] $msg" -ForegroundColor Cyan }
function Write-Ok($msg) { Write-Host "[browsync] $msg" -ForegroundColor Green }
function Write-Err($msg) { Write-Host "[browsync] $msg" -ForegroundColor Red }

Write-Info "browsync installer for Windows"
Write-Info ""

# Detect architecture
$Arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
switch ($Arch) {
    "X64"   { $Target = "x86_64-pc-windows-msvc" }
    "Arm64" { $Target = "aarch64-pc-windows-msvc" }
    default { Write-Err "Unsupported architecture: $Arch"; exit 1 }
}

Write-Info "Detected: Windows $Arch ($Target)"

# Get latest release
Write-Info "Fetching latest release..."
try {
    $Release = Invoke-RestMethod "https://api.github.com/repos/$Repo/releases/latest"
    $Asset = $Release.assets | Where-Object { $_.name -like "*$Target*" } | Select-Object -First 1

    if ($Asset) {
        $DownloadUrl = $Asset.browser_download_url
        Write-Info "Downloading $($Asset.name)..."

        $TmpDir = New-TemporaryFile | ForEach-Object { Remove-Item $_; New-Item -ItemType Directory -Path $_ }
        $ZipPath = Join-Path $TmpDir "browsync.zip"

        Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath
        Expand-Archive -Path $ZipPath -DestinationPath $TmpDir -Force

        # Create install directory
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

        # Copy binaries
        Copy-Item "$TmpDir\browsync.exe" "$InstallDir\browsync.exe" -Force
        if (Test-Path "$TmpDir\browsyncd.exe") {
            Copy-Item "$TmpDir\browsyncd.exe" "$InstallDir\browsyncd.exe" -Force
        }

        # Add to PATH
        $CurrentPath = [Environment]::GetEnvironmentVariable("PATH", "User")
        if ($CurrentPath -notlike "*$InstallDir*") {
            [Environment]::SetEnvironmentVariable("PATH", "$CurrentPath;$InstallDir", "User")
            Write-Info "Added $InstallDir to PATH"
        }

        # Cleanup
        Remove-Item -Recurse -Force $TmpDir

        Write-Ok "browsync installed to $InstallDir"
    }
    else {
        throw "No binary found"
    }
}
catch {
    Write-Info "No prebuilt binary. Building from source..."

    # Check for Rust
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Info "Installing Rust via rustup..."
        Invoke-WebRequest -Uri "https://win.rustup.rs/x86_64" -OutFile "$env:TEMP\rustup-init.exe"
        & "$env:TEMP\rustup-init.exe" -y
        $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    }

    Write-Info "Building browsync from source..."
    cargo install --git "https://github.com/$Repo" browsync-cli
    cargo install --git "https://github.com/$Repo" browsync-daemon

    Write-Ok "Installed via cargo"
}

# Create data directory
$DataDir = "$env:USERPROFILE\.browsync"
New-Item -ItemType Directory -Path $DataDir -Force | Out-Null

Write-Ok ""
Write-Ok "Installation complete!"
Write-Info ""
Write-Info "Get started:"
Write-Info "  browsync detect     # Find installed browsers"
Write-Info "  browsync import     # Import bookmarks & history"
Write-Info "  browsync search     # Search across everything"
Write-Info "  browsync tui        # Launch terminal UI"
Write-Info ""
Write-Info "Restart your terminal for PATH changes to take effect."
