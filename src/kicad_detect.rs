//! Best-effort project-directory auto-detection via KiCad's IPC API.
//! Isolated in its own module -- and, more importantly, behind an
//! opt-in Cargo feature (`kicad-ipc`) -- so a missing or incompatible
//! KiCad install never breaks the rest of the app.
//!
//! `kicad-ipc-rs` pulls in `nng-sys`, which compiles the `nng` C
//! library from source via `cmake`; that's a real system dependency
//! this repo shouldn't force on every build just for this nice-to-have,
//! so it's feature-gated off by default (confirmed in this environment:
//! attempting to build `kicad-ipc-rs` without `cmake` installed fails
//! immediately at its build script, before any of *our* code even
//! compiles).
//!
//! This module could not be exercised in this environment either way
//! (no running KiCad with IPC enabled, and the default build doesn't
//! even compile this path) -- the API calls below (`KiCadClientBlocking
//! ::connect()`, `.get_current_project_path()`) are copied verbatim
//! from `kicad-ipc-rs`'s own README usage examples, not guessed.

use std::path::PathBuf;

/// Tries to ask a running KiCad instance (IPC API enabled, KiCad
/// 10.0.1+) for its current project's directory. Returns `None` on any
/// failure -- not running, IPC not enabled, wrong version, no project
/// open, or (when the `kicad-ipc` feature is disabled) unconditionally,
/// since the app should always work with the user typing the path in
/// manually.
pub fn try_detect_project_dir() -> Option<PathBuf> {
    #[cfg(feature = "kicad-ipc")]
    {
        detect_via_ipc()
    }
    #[cfg(not(feature = "kicad-ipc"))]
    {
        None
    }
}

#[cfg(feature = "kicad-ipc")]
fn detect_via_ipc() -> Option<PathBuf> {
    let client = kicad_ipc_rs::KiCadClientBlocking::connect().ok()?;
    let project_path = client.get_current_project_path().ok()?;
    PathBuf::from(project_path)
        .parent()
        .map(|p| p.to_path_buf())
}
