# openparts-kicad-plugin

A small native GUI companion app for [OpenParts](https://github.com/openparts-org/openparts):
search the OpenParts registry and install a part's Symbol / Footprint / STEP model into a KiCad
project's local libraries.

It does **not** place anything on the schematic or board -- once a part is installed, pick it up
from KiCad's normal Symbol/Footprint picker as usual, and add it to the sheet/board yourself.

## What it does

1. Search the OpenParts registry (`openparts-server`) by manufacturer/MPN.
2. Pick a result (and optionally a silicon revision).
3. Point it at a KiCad project directory -- typed in manually, or auto-filled if a running KiCad
   instance with its IPC API enabled is detected (see [Project-path auto-detect](#project-path-auto-detect)
   below).
4. Click Install. This:
   - downloads the part's KiCad symbol, KiCad footprint, and STEP artifacts (via
     `openparts-client`, which verifies each download's SHA-256 content hash before trusting it),
   - writes the STEP file under `<project>/openparts_3d/<MPN>.step`,
   - writes/merges the footprint into `<project>/openparts.pretty/<MPN>.kicad_mod`, patching its
     `(model ...)` block to point at the downloaded STEP file so the 3D viewer works immediately,
   - merges the symbol into `<project>/openparts.kicad_sym` (creating it if needed, replacing an
     existing entry with the same name if reinstalling),
   - registers both libraries in the project's `sym-lib-table`/`fp-lib-table` if not already
     present.

All of this is idempotent: installing the same part twice updates the existing entries in place
rather than duplicating them.

> **Verified end-to-end** on a real Windows machine with KiCad 10: `install.ps1` downloading and
> installing the binary + launcher, the "OpenParts" toolbar button in the PCB editor, searching,
> installing RP2040, the symbol/footprint libraries being correctly registered as project-specific
> libraries, and the footprint being placed on a real board.
>
> One thing to know: after installing a part for the first time in a KiCad session, **restart the
> Symbol Editor / Footprint Editor / 3D viewer windows** (close and reopen) before the new
> `openparts` library shows up in them. KiCad reads library tables once when each of those windows
> opens and doesn't hot-reload them if the underlying `sym-lib-table`/`fp-lib-table` changes while
> they're already open -- this is KiCad's own behavior, not something this tool can avoid.

## Quick install

Downloads a prebuilt binary and registers the [KiCad toolbar launcher](#launching-from-inside-kicad)
for you -- no Rust toolchain needed.

**Linux:**
```sh
curl -fsSL https://raw.githubusercontent.com/openparts-org/openparts-kicad-plugin/main/install.sh | bash
```

**Windows (PowerShell):**
```powershell
irm https://raw.githubusercontent.com/openparts-org/openparts-kicad-plugin/main/install.ps1 | iex
```
Both scripts try to auto-detect your KiCad plugin directory; if they can't, they'll tell you how to
find it via KiCad's own Tools > External Plugins > Open Plugin Directory and re-run with an explicit
path (`-PluginDir` on Windows, `--plugin-dir` on Linux). Afterwards, restart KiCad (or Tools >
External Plugins > Refresh Plugins) and look for the new "OpenParts" button in the PCB editor
toolbar -- **not** in the Plugin and Content Manager's own "Installed" list, which only tracks
packages it installed itself and never shows an unpackaged Action Plugin like this one.

macOS isn't built by CI yet -- see [Manual install / building from source](#manual-install--building-from-source).

## Manual install / building from source

```sh
cargo build --release
```

This repo depends on `openparts-client`/`openparts-core` from a sibling checkout of
[`openparts`](https://github.com/openparts-org/openparts) via relative path
(`../openparts/crates/...`) -- clone both repos next to each other:

```
OpenParts/
├── openparts/
└── openparts-kicad-plugin/
```

## Running

```sh
cargo run --release
```

Enter your `openparts-server` URL (defaults to `http://localhost:8080`), search, select a part,
set the target project directory, and click Install. The project directory can also be pre-filled
via `--project-dir <path>` on the command line, which is how the KiCad launcher below invokes it.

## Launching from inside KiCad

`kicad-integration/openparts_launcher.py` is a pcbnew Action Plugin: it adds an "OpenParts" button
to the PCB Editor's toolbar that runs this app as an external process with `--project-dir` already
set to the currently open project's directory. It does not use KiCad's IPC API (which has no
library-management support to begin with) -- it's the same "Python toolbar button that shells out
to an external tool" pattern used by plugins like InteractiveHtmlBom.

The [Quick install](#quick-install) scripts set this up automatically, including writing a config
file the launcher reads to find the binary. If you're building from source instead, install it
manually:

```sh
mkdir -p ~/.local/share/kicad/9.0/scripting/plugins
ln -s "$(pwd)/kicad-integration/openparts_launcher.py" \
    ~/.local/share/kicad/9.0/scripting/plugins/openparts_launcher.py
```

Adjust `9.0` to your installed KiCad version (Help > About KiCad; Tools > External Plugins > Open
Plugin Directory is the authoritative way to find this folder on any OS/version), then restart
KiCad or use Tools > External Plugins > Refresh Plugins. The launcher looks for the binary in this
order: the `OPENPARTS_KICAD_PLUGIN_BIN` environment variable, then the installer's config file
(`~/.config/openparts-kicad-plugin/binary_path.txt`, or `%APPDATA%\openparts-kicad-plugin\binary_path.txt`
on Windows), then the `DEFAULT_BINARY_PATH` constant at the top of the script -- edit that constant
if your manual checkout lives somewhere else and you don't want to set an environment variable or
config file.

## Project-path auto-detect (alternative to the launcher above)

Optional feature `kicad-ipc`, off by default:

```sh
cargo build --features kicad-ipc
```

When enabled, the "Detect via KiCad" button uses [`kicad-ipc-rs`](https://docs.rs/kicad-ipc-rs) to
ask a running KiCad instance (10.0.1+, with its IPC API enabled in Preferences) for its current
project path. This is best-effort and never required -- the project directory can always be typed
in by hand, or pre-filled via the launcher plugin above (which needs neither this feature nor
`cmake`). It's off by default because `kicad-ipc-rs` pulls in `nng-sys`, which compiles the `nng`
C library from source via `cmake`; that's a real system dependency not everyone building this app
needs.

## Testing

```sh
cargo test
```

`src/library_sync.rs` (all the `.kicad_sym`/`.kicad_mod`/lib-table file manipulation) and the
`install_part` orchestration in `src/main.rs` are covered by unit/integration tests that run
entirely offline against local fixtures -- no KiCad install or running `openparts-server` needed.

`src/kicad_detect.rs`'s real IPC path (behind the `kicad-ipc` feature) could not be exercised in
the environment this was developed in (no `cmake` available to build `nng-sys`, no running KiCad
instance). Its two API calls (`KiCadClientBlocking::connect()`, `.get_current_project_path()`) are
copied verbatim from `kicad-ipc-rs`'s own documented usage, not guessed -- but this path has only
been verified inside real KiCad by the OpenParts maintainer, not by an automated test here.
