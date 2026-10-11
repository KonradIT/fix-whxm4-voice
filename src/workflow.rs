//! The default command: sets up the workspace for changes and explains the next step.

use std::io;
use std::path::Path;

use crate::changes;
use crate::download::{download_with, downloaded};
use crate::language::Language;
use crate::source::Stock;
use crate::workspace::{FOLDERS, Workspace};

/// Downloads the originals for `lang` from `fetch`, unless the stock pack and every original are
/// already present. Returns the number of clips downloaded, or `None` if the download was skipped.
pub fn prepare(
    ws: &Workspace,
    lang: Language,
    fetch: impl FnOnce(Language) -> io::Result<Stock>,
) -> io::Result<Option<usize>> {
    if downloaded(ws) {
        return Ok(None);
    }
    download_with(ws, lang, fetch).map(Some)
}

/// Tells the user where to put changes or how to generate them, and that running `command`, the
/// default command, again builds and uploads them. The folders are named relative to the workspace,
/// which is named once, unless it is the current directory.
pub fn next_steps(ws: &Workspace, command: &str) -> String {
    let dir = ws.original.parent().unwrap_or(Path::new("."));
    let heading = if dir == Path::new(".") || dir == Path::new("") {
        "Next steps:".to_string()
    } else {
        format!("Next steps, in {}:", dir.display())
    };
    format!(
        "{heading}\n\
         \x20 1. Listen to the originals in {ORIGINAL}/.\n\
         \x20 2. Put your changes in {CHANGES}/.\n\
         \x20    Name each like the original it overrides (only the leading slot number counts).\n\
         \x20    They may be in any audio format. An empty file means silence.\n\
         \x20    Or run `{command} generate` for a ready-made theme of tones.\n\
         \x20 3. Run `{command}` again to build the voice pack and upload it to the headphones.",
        ORIGINAL = FOLDERS[0],
        CHANGES = FOLDERS[1],
    )
}

/// The names of the changes in `changes/`, sorted, which the default command offers to upload.
pub fn change_names(ws: &Workspace) -> io::Result<Vec<String>> {
    Ok(changes::list(&ws.changes)?
        .into_iter()
        .map(|change| change.name)
        .collect())
}

/// Lists the `changes` in `changes/`, which the default command offers to upload.
pub fn change_list(ws: &Workspace, changes: &[String]) -> String {
    let noun = if changes.len() == 1 {
        "change"
    } else {
        "changes"
    };
    let list: Vec<String> = changes.iter().map(|name| format!("\n  {name}")).collect();
    format!(
        "{count} {noun} in {changes_dir}:{list}",
        count = changes.len(),
        changes_dir = ws.changes.display(),
        list = list.concat(),
    )
}

/// Tells the user which voice pack an upload or `build` uses: the pack at `path`, built just now if
/// `rebuilt` or else found up to date, with `changes`. Showing the install language is the upload's
/// job, in its plan.
pub fn build_report(path: &Path, changes: &[String], rebuilt: bool) -> String {
    let what = if rebuilt {
        format!("built and verified {}", path.display())
    } else {
        format!(
            "using {}, not rebuilt because no original or change is newer",
            path.display()
        )
    };
    match changes.len() {
        0 => format!("{what}, with no changes: the originals only"),
        n => {
            let noun = if n == 1 { "change" } else { "changes" };
            let list: Vec<String> = changes.iter().map(|name| format!("\n  {name}")).collect();
            format!("{what}, with {n} {noun}:{}", list.concat())
        }
    }
}
