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

## Building

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
set the target project directory, and click Install.

## Project-path auto-detect

Optional feature `kicad-ipc`, off by default:

```sh
cargo build --features kicad-ipc
```

When enabled, the "Detect via KiCad" button uses [`kicad-ipc-rs`](https://docs.rs/kicad-ipc-rs) to
ask a running KiCad instance (10.0.1+, with its IPC API enabled in Preferences) for its current
project path. This is best-effort and never required -- the project directory can always be typed
in by hand. It's off by default because `kicad-ipc-rs` pulls in `nng-sys`, which compiles the
`nng` C library from source via `cmake`; that's a real system dependency not everyone building
this app needs.

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
