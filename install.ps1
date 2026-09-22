# Installs openparts-kicad-plugin (Windows) and registers its KiCad
# toolbar launcher plugin. Usage:
#
#   irm https://raw.githubusercontent.com/openparts-org/openparts-kicad-plugin/main/install.ps1 | iex
#
# Everything needed is fetched from GitHub Releases -- this script does
# not assume it's run from inside a clone of the repo.
#
# NOTE: this script was written from documented KiCad path conventions
# (see README.md) but could not be executed against a real Windows
# machine or a real KiCad install while developing it -- please report
# any issues you hit running it.

param(
    [string]$PluginDir = ""
)

$ErrorActionPreference = "Stop"

$Repo = "openparts-org/openparts-kicad-plugin"
$Asset = "openparts-kicad-plugin-windows-x86_64.zip"
$InstallDir = Join-Path $env:LOCALAPPDATA "openparts-kicad-plugin"
$ConfigDir = Join-Path $env:APPDATA "openparts-kicad-plugin"

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null

# OPENPARTS_KICAD_PLUGIN_TEST_ZIP is an internal hook for testing this
# script's directory-detection/install logic against a local archive,
# without needing a real GitHub release to exist yet. Not meant for end
# users.
$testZip = $env:OPENPARTS_KICAD_PLUGIN_TEST_ZIP
if ($testZip) {
    $zipPath = $testZip
} else {
    Write-Host "Fetching latest release info for $Repo..."
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    $downloadUrl = ($release.assets | Where-Object { $_.name -eq $Asset } | Select-Object -First 1).browser_download_url

    if (-not $downloadUrl) {
        Write-Error "Could not find a released asset named $Asset. Has a release been published yet? See: https://github.com/$Repo/releases"
        exit 1
    }

    $zipPath = Join-Path $env:TEMP "openparts-kicad-plugin.zip"
    Write-Host "Downloading $Asset..."
    Invoke-WebRequest -Uri $downloadUrl -OutFile $zipPath
}

Expand-Archive -Path $zipPath -DestinationPath $InstallDir -Force
Write-Host "Installed binary to $InstallDir\openparts-kicad-plugin.exe"

# Locate KiCad's plugin directory: highest-version match under
# %APPDATA%\kicad\<version>\scripting\plugins.
function Find-PluginDir {
    $kicadBase = Join-Path $env:APPDATA "kicad"
    if (-not (Test-Path $kicadBase)) {
        return $null
    }
    $versionDirs = Get-ChildItem -Path $kicadBase -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending
    foreach ($v in $versionDirs) {
        $candidate = Join-Path $v.FullName "scripting\plugins"
        if (Test-Path (Split-Path $candidate -Parent)) {
            return $candidate
        }
    }
    return $null
}

if ($PluginDir) {
    $resolvedPluginDir = $PluginDir
} else {
    $resolvedPluginDir = Find-PluginDir
}

if (-not $resolvedPluginDir) {
    Write-Host ""
    Write-Host "Could not auto-detect your KiCad plugin directory."
    Write-Host "In KiCad: Tools > External Plugins > Open Plugin Directory, then download and"
    Write-Host "re-run this script with that path (piping to iex can't pass parameters):"
    Write-Host "  Invoke-WebRequest -Uri https://raw.githubusercontent.com/$Repo/main/install.ps1 -OutFile install.ps1"
    Write-Host "  .\install.ps1 -PluginDir <path>"
    exit 1
}

New-Item -ItemType Directory -Force -Path $resolvedPluginDir | Out-Null
Copy-Item -Path (Join-Path $InstallDir "openparts_launcher.py") -Destination $resolvedPluginDir -Force
Write-Host "Registered KiCad launcher plugin in $resolvedPluginDir"

New-Item -ItemType Directory -Force -Path $ConfigDir | Out-Null
Set-Content -Path (Join-Path $ConfigDir "binary_path.txt") -Value (Join-Path $InstallDir "openparts-kicad-plugin.exe") -NoNewline

Write-Host ""
Write-Host "Done. Restart KiCad, or use Tools > External Plugins > Refresh Plugins,"
Write-Host "then look for the new `"OpenParts`" button in the PCB editor toolbar."
