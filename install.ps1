# Installs openparts-kicad-plugin (Windows) and registers its KiCad
# toolbar launcher plugin. Usage:
#
#   irm https://raw.githubusercontent.com/openparts-org/openparts-kicad-plugin/main/install.ps1 | iex
#
# Everything needed is fetched from GitHub Releases -- this script does
# not assume it's run from inside a clone of the repo.
#
# NOTE: the download/install steps have been confirmed working on a
# real Windows machine with KiCad 10. Plugin-directory auto-detection
# was corrected from that real feedback (KiCad 10 moved user data to
# Documents\KiCad\<version>\3rdparty\plugins) but the corrected
# auto-detection itself hasn't been re-confirmed yet -- if it doesn't
# find your plugin directory, use -PluginDir with the path from KiCad's
# own Tools > External Plugins > Open Plugin Directory and please
# report it.

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

# Locate KiCad's plugin directory. Confirmed on a real KiCad 10 install
# (Windows): user data moved to Documents\KiCad\<version>\, and
# unpackaged Action Plugins (like this one) live under
# Documents\KiCad\<version>\3rdparty\plugins -- note this is a
# different, unrelated concept from the Plugin and Content Manager's
# own "Installed" list, which only tracks packages it installed itself
# and will never show a loose script dropped into this folder.
# %APPDATA%\kicad\<version>\scripting\plugins (older/legacy layout) is
# kept as a fallback for older KiCad versions.
function Find-PluginDir {
    $documents = [Environment]::GetFolderPath("MyDocuments")
    $bases = @(
        @{ Root = Join-Path $documents "KiCad"; SubPath = "3rdparty\plugins" },
        @{ Root = Join-Path $env:APPDATA "kicad"; SubPath = "scripting\plugins" }
    )
    foreach ($base in $bases) {
        if (-not (Test-Path $base.Root)) {
            continue
        }
        $versionDirs = Get-ChildItem -Path $base.Root -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending
        foreach ($v in $versionDirs) {
            $candidate = Join-Path $v.FullName $base.SubPath
            if (Test-Path (Split-Path $candidate -Parent)) {
                return $candidate
            }
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
