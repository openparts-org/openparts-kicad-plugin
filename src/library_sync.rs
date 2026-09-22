//! Pure file-manipulation logic: no `eframe`/`egui`, no KiCad API --
//! everything here operates on plain strings and is fully unit-testable
//! in this environment. `sym-lib-table`/`fp-lib-table` and
//! `.kicad_sym`/`.kicad_mod` are themselves simple S-expression text
//! files (same format `openparts-kicad` already emits), so none of this
//! needs a KiCad install or library.
//!
//! The block-extraction/insertion here is deliberately a simple
//! balanced-paren scanner, not a general S-expression parser -- the
//! same "our own generator's output is well-formed, so scanning
//! suffices" approach `openparts-kicad`'s read-back parsers already use,
//! since every input this module reads was itself produced by
//! `openparts-kicad`'s renderer.

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("could not find a balanced `{0}` block in the input")]
    BlockNotFound(String),
}

/// Finds the byte range `[start, end)` of the first balanced
/// parenthesized block whose text begins with `marker` (which must
/// itself start with `(`).
fn find_balanced_block(text: &str, marker: &str) -> Option<(usize, usize)> {
    let start = text.find(marker)?;
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((start, i + 1));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Extracts just the `(symbol "MPN" ...)` block out of a full
/// single-symbol `.kicad_sym` library file as returned by
/// `openparts-server`'s kicad-symbol artifact.
pub fn extract_symbol_block(fetched_symbol_lib_text: &str, mpn: &str) -> Result<String, SyncError> {
    let marker = format!("(symbol \"{mpn}\"");
    let (start, end) = find_balanced_block(fetched_symbol_lib_text, &marker)
        .ok_or_else(|| SyncError::BlockNotFound(marker.clone()))?;
    Ok(fetched_symbol_lib_text[start..end].to_string())
}

/// Merges `symbol_block` (as returned by [`extract_symbol_block`]) into
/// `existing` (the project's own combined `openparts.kicad_sym`, or
/// `None` if it doesn't exist yet). If a symbol with the same name is
/// already present, it's replaced in place (idempotent: installing the
/// same part twice doesn't duplicate it).
pub fn merge_symbol_library(existing: Option<&str>, mpn: &str, symbol_block: &str) -> String {
    let marker = format!("(symbol \"{mpn}\"");
    match existing {
        Some(text) => {
            if let Some((start, end)) = find_balanced_block(text, &marker) {
                format!("{}{}{}", &text[..start], symbol_block, &text[end..])
            } else {
                // Insert just before the final closing paren of the
                // (kicad_symbol_lib ...) wrapper.
                let trimmed = text.trim_end();
                let insert_at = trimmed.rfind(')').unwrap_or(trimmed.len());
                format!("{}  {}\n{}", &trimmed[..insert_at], symbol_block, &trimmed[insert_at..])
            }
        }
        None => format!(
            "(kicad_symbol_lib\n  (version 20211014)\n  (generator openparts-kicad-plugin)\n  {symbol_block}\n)\n"
        ),
    }
}

/// Inserts a `(model "path" ...)` block into a footprint (as returned by
/// `openparts-server`'s kicad-footprint artifact) pointing at the
/// downloaded STEP file, so the 3D viewer works immediately. If the
/// footprint already has a `(model ...)` block, it's replaced.
pub fn patch_footprint_3d_model(footprint_text: &str, step_relative_path: &str) -> String {
    let model_block = format!(
        "  (model \"{step_relative_path}\"\n    (offset (xyz 0 0 0))\n    (scale (xyz 1 1 1))\n    (rotate (xyz 0 0 0))\n  )\n"
    );
    if let Some((start, end)) = find_balanced_block(footprint_text, "(model ") {
        format!(
            "{}{}{}",
            &footprint_text[..start],
            model_block.trim_end(),
            &footprint_text[end..]
        )
    } else {
        let trimmed = footprint_text.trim_end();
        let insert_at = trimmed.rfind(')').unwrap_or(trimmed.len());
        format!(
            "{}{}{}",
            &trimmed[..insert_at],
            model_block,
            &trimmed[insert_at..]
        )
    }
}

/// Adds a `(lib (name "openparts") ...)` row to a `sym-lib-table` or
/// `fp-lib-table` file (same format for both). `existing` is `None` if
/// the project has no such table file yet. Idempotent: if a row with
/// `lib_name` already exists, it's left untouched (not duplicated).
pub fn add_lib_table_row(
    existing: Option<&str>,
    table_tag: &str,
    lib_name: &str,
    lib_type: &str,
    uri: &str,
) -> String {
    let name_marker = format!("(name \"{lib_name}\")");
    if let Some(text) = existing {
        if text.contains(&name_marker) {
            return text.to_string();
        }
        let row = format!(
            "  (lib (name \"{lib_name}\")(type \"{lib_type}\")(uri \"{uri}\")(options \"\")(descr \"\"))\n"
        );
        let trimmed = text.trim_end();
        let insert_at = trimmed.rfind(')').unwrap_or(trimmed.len());
        format!("{}{}{}", &trimmed[..insert_at], row, &trimmed[insert_at..])
    } else {
        format!(
            "({table_tag}\n  (lib (name \"{lib_name}\")(type \"{lib_type}\")(uri \"{uri}\")(options \"\")(descr \"\"))\n)\n"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FETCHED_SYMBOL_LIB: &str = "(kicad_symbol_lib\n  (version 20211014)\n  (generator openparts)\n  (symbol \"RP2040\"\n    (in_bom yes) (on_board yes)\n    (symbol \"RP2040_1_1\"\n      (pin power_in line (at 0 0 0) (length 2.54)\n        (name \"IOVDD\" (effects (font (size 1.27 1.27))))\n        (number \"1\" (effects (font (size 1.27 1.27)))))\n    )\n  )\n)\n";

    #[test]
    fn extracts_the_symbol_block_from_a_fetched_library() {
        let block = extract_symbol_block(FETCHED_SYMBOL_LIB, "RP2040").unwrap();
        assert!(block.starts_with("(symbol \"RP2040\""));
        assert!(block.trim_end().ends_with(')'));
        // Balanced: same number of ( and ).
        assert_eq!(block.matches('(').count(), block.matches(')').count());
    }

    #[test]
    fn merge_creates_a_new_library_when_none_exists() {
        let block = extract_symbol_block(FETCHED_SYMBOL_LIB, "RP2040").unwrap();
        let merged = merge_symbol_library(None, "RP2040", &block);
        assert!(merged.starts_with("(kicad_symbol_lib"));
        assert!(merged.contains("(symbol \"RP2040\""));
    }

    #[test]
    fn merge_appends_to_an_existing_library_without_disturbing_other_symbols() {
        let existing = "(kicad_symbol_lib\n  (version 20211014)\n  (generator openparts-kicad-plugin)\n  (symbol \"OTHER_PART\"\n    (in_bom yes)\n  )\n)\n";
        let block = extract_symbol_block(FETCHED_SYMBOL_LIB, "RP2040").unwrap();
        let merged = merge_symbol_library(Some(existing), "RP2040", &block);
        assert!(merged.contains("(symbol \"OTHER_PART\""));
        assert!(merged.contains("(symbol \"RP2040\""));
    }

    #[test]
    fn merge_is_idempotent_installing_twice_does_not_duplicate() {
        let block = extract_symbol_block(FETCHED_SYMBOL_LIB, "RP2040").unwrap();
        let once = merge_symbol_library(None, "RP2040", &block);
        let twice = merge_symbol_library(Some(&once), "RP2040", &block);
        assert_eq!(twice.matches("(symbol \"RP2040\"").count(), 1);
    }

    const FETCHED_FOOTPRINT: &str = "(footprint \"RP2040\" (version 20221018) (generator openparts)\n  (layer \"F.Cu\")\n  (pad \"1\" smd rect (at 0 0) (size 0.4 0.25) (layers \"F.Cu\"))\n)\n";

    #[test]
    fn patches_3d_model_into_a_footprint_with_none() {
        let patched =
            patch_footprint_3d_model(FETCHED_FOOTPRINT, "${KIPRJMOD}/openparts_3d/RP2040.step");
        assert!(patched.contains("(model \"${KIPRJMOD}/openparts_3d/RP2040.step\""));
        // Still balanced overall.
        assert_eq!(patched.matches('(').count(), patched.matches(')').count());
    }

    #[test]
    fn patching_twice_replaces_rather_than_duplicates() {
        let once = patch_footprint_3d_model(FETCHED_FOOTPRINT, "path/a.step");
        let twice = patch_footprint_3d_model(&once, "path/b.step");
        assert_eq!(twice.matches("(model ").count(), 1);
        assert!(twice.contains("path/b.step"));
        assert!(!twice.contains("path/a.step"));
    }

    #[test]
    fn lib_table_creates_new_file_when_none_exists() {
        let table = add_lib_table_row(
            None,
            "sym_lib_table",
            "openparts",
            "KiCad",
            "${KIPRJMOD}/openparts.kicad_sym",
        );
        assert!(table.starts_with("(sym_lib_table"));
        assert!(table.contains("(name \"openparts\")"));
    }

    #[test]
    fn lib_table_preserves_other_libraries() {
        let existing = "(fp_lib_table\n  (lib (name \"Other\")(type \"KiCad\")(uri \"other\")(options \"\")(descr \"\"))\n)\n";
        let updated = add_lib_table_row(
            Some(existing),
            "fp_lib_table",
            "openparts",
            "KiCad",
            "${KIPRJMOD}/openparts.pretty",
        );
        assert!(updated.contains("(name \"Other\")"));
        assert!(updated.contains("(name \"openparts\")"));
    }

    #[test]
    fn lib_table_is_idempotent() {
        let once = add_lib_table_row(None, "sym_lib_table", "openparts", "KiCad", "uri");
        let twice = add_lib_table_row(Some(&once), "sym_lib_table", "openparts", "KiCad", "uri");
        assert_eq!(twice.matches("(name \"openparts\")").count(), 1);
    }
}
