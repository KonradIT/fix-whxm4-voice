//! The default command: sets up the workspace for changes and explains the next step.

use std::io;

use crate::download::{download_with, downloaded};
use crate::language::Language;
use crate::source::Stock;
use crate::workspace::Workspace;

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

/// Tells the user where to put changes and that `upload_command` uploads them.
pub fn next_steps(ws: &Workspace, upload_command: &str) -> String {
    format!(
        "Next steps:\n\
         \x20 1. Listen to the originals in {original}.\n\
         \x20 2. Put your changes in {changes}.\n\
         \x20    Name each like the original it overrides (only the leading slot number counts).\n\
         \x20    They may be in any audio format. An empty file means silence.\n\
         \x20 3. Run `{upload_command}` to build the voice pack and upload it to the headphones.",
        original = ws.original.display(),
        changes = ws.changes.display(),
    )
}
