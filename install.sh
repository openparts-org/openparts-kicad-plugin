#!/usr/bin/env bash
# Installs openparts-kicad-plugin (Linux) and registers its KiCad toolbar
# launcher plugin. Usage:
#
#   curl -fsSL https://raw.githubusercontent.com/openparts-org/openparts-kicad-plugin/main/install.sh | bash
#
# Everything needed is fetched from GitHub Releases -- this script does
# not assume it's run from inside a clone of the repo.
set -euo pipefail

REPO="openparts-org/openparts-kicad-plugin"
INSTALL_DIR="${OPENPARTS_KICAD_PLUGIN_INSTALL_DIR:-$HOME/.local/share/openparts-kicad-plugin}"
CONFIG_DIR="$HOME/.config/openparts-kicad-plugin"

plugin_dir_override=""
while [ $# -gt 0 ]; do
  case "$1" in
    --plugin-dir)
      plugin_dir_override="$2"
      shift 2
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

os="$(uname -s)"
case "$os" in
  Linux)
    asset="openparts-kicad-plugin-linux-x86_64.tar.gz"
    ;;
  *)
    echo "openparts-kicad-plugin does not publish prebuilt binaries for '$os' yet." >&2
    echo "Build from source instead: see README.md's 'Manual install / building from source' section." >&2
    exit 1
    ;;
esac

mkdir -p "$INSTALL_DIR"

# OPENPARTS_KICAD_PLUGIN_TEST_TARBALL is an internal hook for testing
# this script's directory-detection/install logic against a local
# archive, without needing a real GitHub release to exist yet. Not
# meant for end users. Kept out of the mktemp/trap cleanup below since
# it's a caller-owned file, not one this script created.
if [ -n "${OPENPARTS_KICAD_PLUGIN_TEST_TARBALL:-}" ]; then
  archive="$OPENPARTS_KICAD_PLUGIN_TEST_TARBALL"
else
  tmp_archive="$(mktemp)"
  trap 'rm -f "$tmp_archive"' EXIT
  archive="$tmp_archive"

  echo "Fetching latest release info for $REPO..."
  release_json="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest")"
  download_url="$(printf '%s' "$release_json" | grep -o "\"browser_download_url\": *\"[^\"]*${asset}\"" | head -n1 | sed -E 's/.*"(https[^"]+)"/\1/')"

  if [ -z "$download_url" ]; then
    echo "Could not find a released asset named $asset. Has a release been published yet?" >&2
    echo "See: https://github.com/$REPO/releases" >&2
    exit 1
  fi

  echo "Downloading $asset..."
  curl -fsSL "$download_url" -o "$archive"
fi

tar -xzf "$archive" -C "$INSTALL_DIR"
chmod +x "$INSTALL_DIR/openparts-kicad-plugin"

echo "Installed binary to $INSTALL_DIR/openparts-kicad-plugin"

# Locate KiCad's plugin directory: highest-version match under any of
# the known scripting/plugins or PCM 3rdparty/plugins layouts.
find_plugin_dir() {
  local kicad_base candidate best=""
  for kicad_base in "$HOME/.local/share/kicad" "$HOME/.config/kicad"; do
    [ -d "$kicad_base" ] || continue
    for candidate in "$kicad_base"/*/scripting/plugins "$kicad_base"/*/3rdparty/plugins; do
      [ -d "$(dirname "$candidate")" ] || continue
      best="$candidate"
    done
  done
  printf '%s' "$best"
}

if [ -n "$plugin_dir_override" ]; then
  plugin_dir="$plugin_dir_override"
else
  plugin_dir="$(find_plugin_dir)"
fi

if [ -z "$plugin_dir" ]; then
  echo ""
  echo "Could not auto-detect your KiCad plugin directory."
  echo "In KiCad: Tools > External Plugins > Open Plugin Directory, then re-run:"
  echo "  curl -fsSL https://raw.githubusercontent.com/$REPO/main/install.sh | bash -s -- --plugin-dir <path>"
  exit 1
fi

mkdir -p "$plugin_dir"
cp "$INSTALL_DIR/openparts_launcher.py" "$plugin_dir/openparts_launcher.py"
echo "Registered KiCad launcher plugin in $plugin_dir"

mkdir -p "$CONFIG_DIR"
printf '%s' "$INSTALL_DIR/openparts-kicad-plugin" > "$CONFIG_DIR/binary_path.txt"

echo ""
echo "Done. Restart KiCad, or use Tools > External Plugins > Refresh Plugins,"
echo "then look for the new \"OpenParts\" button in the PCB editor toolbar."
