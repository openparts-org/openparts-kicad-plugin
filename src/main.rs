mod kicad_detect;
mod library_sync;

use eframe::egui;
use openparts_client::{ArtifactKind, Client, PartSummary};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

/// How long to wait after the last keystroke before firing an automatic
/// (type-ahead) search -- the explicit Search button and Enter key
/// always bypass this and fire immediately.
const DEBOUNCE: Duration = Duration::from_millis(250);
/// Minimum query length before an automatic search fires -- avoids a
/// near-useless "everything matches" hit on the very first keystroke.
/// Does not apply to an explicit Search-button/Enter dispatch.
const MIN_QUERY_LEN: usize = 2;

fn main() -> eframe::Result<()> {
    let project_dir = parse_project_dir_arg(std::env::args());
    eframe::run_native(
        "OpenParts for KiCad",
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(App::new(project_dir)))),
    )
}

/// Parses an optional `--project-dir <path>` / `--project-dir=<path>`
/// argument, used by `kicad-integration/openparts_launcher.py` to
/// pre-fill the project directory with KiCad's own currently-open
/// project path, so the user doesn't have to type or paste it in by
/// hand when launched from KiCad's toolbar.
fn parse_project_dir_arg(args: impl Iterator<Item = String>) -> Option<String> {
    let mut args = args.skip(1);
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--project-dir=") {
            return Some(value.to_string());
        }
        if arg == "--project-dir" {
            return args.next();
        }
    }
    None
}

type SearchResult = (u64, Result<Vec<PartSummary>, String>);

struct App {
    registry_url: String,
    query: String,
    results: Vec<PartSummary>,
    selected: Option<usize>,
    revision: String,
    project_dir: String,
    status: String,
    search_tx: Sender<SearchResult>,
    search_rx: Receiver<SearchResult>,
    /// Incremented on every dispatched search; a background search's
    /// reply is only applied if it's still the latest one dispatched --
    /// this discards a slow, superseded request's late-arriving
    /// response instead of flashing stale results over newer ones.
    next_generation: u64,
    latest_dispatched_generation: u64,
    /// Set when `query` changes and hasn't yet triggered an automatic
    /// dispatch; cleared once a search (automatic or explicit) fires --
    /// this is what prevents re-dispatching every frame once the
    /// debounce condition holds.
    last_edit_at: Option<Instant>,
}

impl App {
    fn new(project_dir: Option<String>) -> Self {
        let (search_tx, search_rx) = std::sync::mpsc::channel();
        Self {
            registry_url: "http://localhost:8080".to_string(),
            query: String::new(),
            results: Vec::new(),
            selected: None,
            revision: String::new(),
            project_dir: project_dir.unwrap_or_default(),
            status: String::new(),
            search_tx,
            search_rx,
            next_generation: 0,
            latest_dispatched_generation: 0,
            last_edit_at: None,
        }
    }
}

/// Pure decision logic for automatic (debounced) search dispatch, kept
/// separate from `App::update` so it's testable without an egui
/// context, a background thread, or a live server.
fn should_auto_dispatch(elapsed_since_edit: Duration, query_len_chars: usize) -> bool {
    elapsed_since_edit >= DEBOUNCE && query_len_chars >= MIN_QUERY_LEN
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain any background search replies, applying only the latest
        // dispatched generation's result (see `next_generation`'s docs).
        while let Ok((generation, result)) = self.search_rx.try_recv() {
            if generation == self.latest_dispatched_generation {
                match result {
                    Ok(results) => {
                        self.status = format!("{} result(s).", results.len());
                        self.results = results;
                        self.selected = None;
                    }
                    Err(e) => {
                        self.status = format!("Search failed: {e}");
                        self.results.clear();
                    }
                }
            }
        }

        // Debounced automatic dispatch: fires `DEBOUNCE` after the last
        // keystroke, provided the query is long enough. `request_repaint_after`
        // is required here -- egui only redraws on input/explicit request by
        // default, so without it the app would never wake up on its own to
        // notice the debounce timer has elapsed while the user is idle.
        if let Some(edit_at) = self.last_edit_at {
            let elapsed = edit_at.elapsed();
            if should_auto_dispatch(elapsed, self.query.chars().count()) {
                self.dispatch_search(ctx);
            } else {
                ctx.request_repaint_after(DEBOUNCE.saturating_sub(elapsed));
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("OpenParts for KiCad");
            ui.label(
                "Search OpenParts and install a part's Symbol/Footprint/STEP into a KiCad \
                 project's local libraries. Placing it on the schematic/board is done as usual \
                 in KiCad afterwards.",
            );
            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Registry URL:");
                ui.text_edit_singleline(&mut self.registry_url);
            });

            ui.horizontal(|ui| {
                ui.label("Search:");
                let response = ui.text_edit_singleline(&mut self.query);
                if response.changed() {
                    self.last_edit_at = Some(Instant::now());
                }
                let submitted_via_enter =
                    response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if submitted_via_enter || ui.button("Search").clicked() {
                    self.dispatch_search(ctx);
                }
            });

            ui.separator();
            for (i, result) in self.results.iter().enumerate() {
                let label = format!(
                    "{} / {}  [{:?} / {:?}]",
                    result.manufacturer, result.mpn, result.existence, result.lifecycle
                );
                if ui
                    .selectable_label(self.selected == Some(i), label)
                    .clicked()
                {
                    self.selected = Some(i);
                }
            }

            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Silicon revision (optional):");
                ui.text_edit_singleline(&mut self.revision);
            });
            ui.horizontal(|ui| {
                ui.label("Project directory:");
                ui.text_edit_singleline(&mut self.project_dir);
                if ui.button("Detect via KiCad").clicked() {
                    match kicad_detect::try_detect_project_dir() {
                        Some(dir) => {
                            self.project_dir = dir.display().to_string();
                            self.status = "Detected project directory via KiCad IPC.".to_string();
                        }
                        None => {
                            self.status =
                                "Could not auto-detect (KiCad IPC unavailable) -- enter the \
                                 project directory manually."
                                    .to_string();
                        }
                    }
                }
            });

            if ui.button("Install").clicked() {
                self.do_install();
            }

            ui.separator();
            ui.label(&self.status);
        });
    }
}

impl App {
    /// Fires a search in the background (never blocks the UI thread --
    /// `Client::search` is a blocking HTTP call). Called both for
    /// automatic (debounced) type-ahead dispatch and for an explicit
    /// Search-button/Enter submission, which bypass the debounce delay
    /// and `MIN_QUERY_LEN` check by calling this directly.
    fn dispatch_search(&mut self, ctx: &egui::Context) {
        self.next_generation += 1;
        let generation = self.next_generation;
        self.latest_dispatched_generation = generation;
        self.last_edit_at = None;
        self.status = "Searching...".to_string();

        let registry_url = self.registry_url.clone();
        let query = self.query.clone();
        let tx = self.search_tx.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let client = Client::new(&registry_url);
            let result = client.search(&query).map_err(|e| e.to_string());
            let _ = tx.send((generation, result));
            // Background results arrive outside egui's normal
            // input-driven redraw cycle -- explicitly request a repaint
            // so the result shows up promptly instead of waiting for
            // the next user interaction.
            ctx.request_repaint();
        });
    }

    fn do_install(&mut self) {
        let Some(index) = self.selected else {
            self.status = "Select a search result first.".to_string();
            return;
        };
        let result = &self.results[index];
        let manufacturer = result.manufacturer.clone();
        let mpn = result.mpn.clone();
        let revision = if self.revision.trim().is_empty() {
            None
        } else {
            Some(self.revision.trim().to_string())
        };
        if self.project_dir.trim().is_empty() {
            self.status = "Set a project directory first.".to_string();
            return;
        }
        let project_dir = PathBuf::from(self.project_dir.trim());

        let client = Client::new(&self.registry_url);
        match install_part(
            &client,
            &manufacturer,
            &mpn,
            revision.as_deref(),
            &project_dir,
        ) {
            Ok(message) => self.status = message,
            Err(e) => self.status = format!("Install failed: {e}"),
        }
    }
}

/// Fetches the three artifacts and writes/merges them into `project_dir`'s
/// local libraries. Thin orchestration over `library_sync`'s pure logic
/// and `openparts_client`'s already hash-verified downloads.
fn install_part(
    client: &Client,
    manufacturer: &str,
    mpn: &str,
    revision: Option<&str>,
    project_dir: &Path,
) -> Result<String, String> {
    let symbol_artifact = client
        .get_artifact(manufacturer, mpn, ArtifactKind::KicadSymbol, revision)
        .map_err(|e| e.to_string())?;
    let footprint_artifact = client
        .get_artifact(manufacturer, mpn, ArtifactKind::KicadFootprint, revision)
        .map_err(|e| e.to_string())?;
    let step_artifact = client
        .get_artifact(manufacturer, mpn, ArtifactKind::Step, revision)
        .map_err(|e| e.to_string())?;

    let step_dir = project_dir.join("openparts_3d");
    std::fs::create_dir_all(&step_dir).map_err(|e| e.to_string())?;
    std::fs::write(step_dir.join(format!("{mpn}.step")), &step_artifact.content)
        .map_err(|e| e.to_string())?;

    let step_rel = format!("${{KIPRJMOD}}/openparts_3d/{mpn}.step");
    let patched_footprint =
        library_sync::patch_footprint_3d_model(&footprint_artifact.content, &step_rel);
    let pretty_dir = project_dir.join("openparts.pretty");
    std::fs::create_dir_all(&pretty_dir).map_err(|e| e.to_string())?;
    std::fs::write(
        pretty_dir.join(format!("{mpn}.kicad_mod")),
        patched_footprint,
    )
    .map_err(|e| e.to_string())?;

    let symbol_block = library_sync::extract_symbol_block(&symbol_artifact.content, mpn)
        .map_err(|e| e.to_string())?;
    let sym_path = project_dir.join("openparts.kicad_sym");
    let existing_sym = std::fs::read_to_string(&sym_path).ok();
    let merged_sym =
        library_sync::merge_symbol_library(existing_sym.as_deref(), mpn, &symbol_block);
    std::fs::write(&sym_path, merged_sym).map_err(|e| e.to_string())?;

    let sym_table_path = project_dir.join("sym-lib-table");
    let existing_sym_table = std::fs::read_to_string(&sym_table_path).ok();
    let updated_sym_table = library_sync::add_lib_table_row(
        existing_sym_table.as_deref(),
        "sym_lib_table",
        "openparts",
        "KiCad",
        "${KIPRJMOD}/openparts.kicad_sym",
    );
    std::fs::write(&sym_table_path, updated_sym_table).map_err(|e| e.to_string())?;

    let fp_table_path = project_dir.join("fp-lib-table");
    let existing_fp_table = std::fs::read_to_string(&fp_table_path).ok();
    let updated_fp_table = library_sync::add_lib_table_row(
        existing_fp_table.as_deref(),
        "fp_lib_table",
        "openparts",
        "KiCad",
        "${KIPRJMOD}/openparts.pretty",
    );
    std::fs::write(&fp_table_path, updated_fp_table).map_err(|e| e.to_string())?;

    Ok(format!(
        "Installed {mpn} into {} (symbol hash sha256:{}, footprint hash sha256:{}, step hash sha256:{})",
        project_dir.display(),
        symbol_artifact.hash,
        footprint_artifact.hash,
        step_artifact.hash
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_auto_dispatch_before_the_debounce_elapses() {
        assert!(!should_auto_dispatch(
            Duration::from_millis(100),
            "STM".len()
        ));
    }

    #[test]
    fn auto_dispatches_once_debounce_elapses_with_a_long_enough_query() {
        assert!(should_auto_dispatch(Duration::from_millis(300), "ST".len()));
    }

    #[test]
    fn does_not_auto_dispatch_a_too_short_query_even_after_debounce_elapses() {
        assert!(!should_auto_dispatch(Duration::from_millis(300), "S".len()));
    }

    #[test]
    fn empty_query_never_auto_dispatches() {
        assert!(!should_auto_dispatch(Duration::from_secs(10), 0));
    }

    #[test]
    fn project_dir_arg_absent_yields_none() {
        let args = ["openparts-kicad-plugin".to_string()];
        assert_eq!(parse_project_dir_arg(args.into_iter()), None);
    }

    #[test]
    fn project_dir_arg_space_separated() {
        let args = [
            "openparts-kicad-plugin".to_string(),
            "--project-dir".to_string(),
            "/home/user/my-project".to_string(),
        ];
        assert_eq!(
            parse_project_dir_arg(args.into_iter()),
            Some("/home/user/my-project".to_string())
        );
    }

    #[test]
    fn project_dir_arg_equals_separated() {
        let args = [
            "openparts-kicad-plugin".to_string(),
            "--project-dir=/home/user/my-project".to_string(),
        ];
        assert_eq!(
            parse_project_dir_arg(args.into_iter()),
            Some("/home/user/my-project".to_string())
        );
    }

    /// Exercises `install_part`'s file-writing side entirely against a
    /// local filesystem fixture -- no real `openparts-server` needed,
    /// since `Client` itself already has its own network tests
    /// (openparts-client's dependency-free mock-server suite). This
    /// just confirms the orchestration + library_sync wiring is
    /// correct by pre-populating the client's cache and running fully
    /// offline.
    #[test]
    fn install_part_writes_expected_files_offline_from_cache() {
        let cache_dir = tempfile::tempdir().unwrap();
        let project_dir = tempfile::tempdir().unwrap();

        // Pre-populate the client cache exactly like openparts-client's
        // own cache layout (metadata/artifact-pointers + artifacts/sha256).
        let symbol_text = "(kicad_symbol_lib\n  (version 20211014)\n  (generator openparts)\n  (symbol \"TESTPART\"\n    (in_bom yes)\n  )\n)\n";
        let footprint_text = "(footprint \"TESTPART\" (version 20221018) (generator openparts)\n  (layer \"F.Cu\")\n  (pad \"1\" smd rect (at 0 0) (size 0.4 0.25) (layers \"F.Cu\"))\n)\n";
        let step_text = "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n";

        write_cached_artifact(
            cache_dir.path(),
            "ex",
            "TESTPART",
            "kicad-symbol",
            symbol_text,
        );
        write_cached_artifact(
            cache_dir.path(),
            "ex",
            "TESTPART",
            "kicad-footprint",
            footprint_text,
        );
        write_cached_artifact(cache_dir.path(), "ex", "TESTPART", "step", step_text);

        let client = Client::new("http://127.0.0.1:1")
            .with_cache_dir(cache_dir.path().to_path_buf())
            .offline(true);

        let message = install_part(&client, "ex", "TESTPART", None, project_dir.path()).unwrap();
        assert!(message.contains("Installed TESTPART"));

        assert!(project_dir
            .path()
            .join("openparts_3d/TESTPART.step")
            .exists());
        assert!(project_dir
            .path()
            .join("openparts.pretty/TESTPART.kicad_mod")
            .exists());
        assert!(project_dir.path().join("openparts.kicad_sym").exists());

        let footprint_out = std::fs::read_to_string(
            project_dir
                .path()
                .join("openparts.pretty/TESTPART.kicad_mod"),
        )
        .unwrap();
        assert!(footprint_out.contains("openparts_3d/TESTPART.step"));

        let sym_table = std::fs::read_to_string(project_dir.path().join("sym-lib-table")).unwrap();
        assert!(sym_table.contains("(name \"openparts\")"));
        let fp_table = std::fs::read_to_string(project_dir.path().join("fp-lib-table")).unwrap();
        assert!(fp_table.contains("(name \"openparts\")"));
    }

    fn write_cached_artifact(
        cache_dir: &Path,
        manufacturer: &str,
        mpn: &str,
        kind: &str,
        content: &str,
    ) {
        use sha2::{Digest, Sha256};
        let hash: String = {
            let mut h = Sha256::new();
            h.update(content.as_bytes());
            h.finalize().iter().map(|b| format!("{b:02x}")).collect()
        };
        let pointer_path = cache_dir.join(format!(
            "metadata/artifact-pointers/{manufacturer}/{mpn}/{kind}.hash"
        ));
        std::fs::create_dir_all(pointer_path.parent().unwrap()).unwrap();
        std::fs::write(&pointer_path, &hash).unwrap();
        let content_path = cache_dir.join(format!("artifacts/sha256/{hash}"));
        std::fs::create_dir_all(content_path.parent().unwrap()).unwrap();
        std::fs::write(&content_path, content).unwrap();
    }
}
