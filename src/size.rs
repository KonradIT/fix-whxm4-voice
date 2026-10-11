//! The size check: refuses a pack whose clips are larger than the limit, reporting every clip.

use std::fmt::Write;
use std::path::Path;

use crate::changes::Change;
use crate::slots::clip_file_name;
use crate::workspace::Workspace;

/// The most bytes the clips of a pack may take together.
pub struct Limit {
    pub bytes: usize,
    /// Where the limit comes from, as the report names it.
    pub source: &'static str,
}

/// The limit for a pack built from the stock clips `stock`: their total size, which the headphones
/// are known to accept. This is the one place the limit is decided.
pub fn limit(stock: &[Vec<u8>]) -> Limit {
    Limit {
        bytes: stock.iter().map(Vec::len).sum(),
        source: "the total of the stock clips of the base language",
    }
}

/// Checks that `clips`, the converted clip of every slot as `resolved` gives it, fit in `limit`. If
/// they don't, returns a report listing every clip, the total and the limit, and asking for the
/// changes to be trimmed.
pub fn check(
    clips: &[Vec<u8>],
    resolved: &[Change],
    limit: &Limit,
    ws: &Workspace,
) -> Result<(), String> {
    let total: usize = clips.iter().map(Vec::len).sum();
    if total <= limit.bytes {
        return Ok(());
    }

    let mut report = format!(
        "the clips take {total} bytes, {} more than the limit of {} bytes (changes are marked *):\n",
        total - limit.bytes,
        limit.bytes
    );
    for (slot, (clip, change)) in clips.iter().zip(resolved).enumerate() {
        let (mark, clip_name) = match change {
            Change::Original => (' ', path(&ws.original, &clip_file_name(slot))),
            Change::Replace(name) => ('*', path(&ws.changes, name)),
            Change::Silence => (
                '*',
                format!(
                    "silence in place of {}",
                    path(&ws.original, &clip_file_name(slot))
                ),
            ),
        };
        writeln!(report, "  {mark} {:>7}  {clip_name}", clip.len()).unwrap();
    }
    writeln!(report, "    total {total}").unwrap();
    writeln!(report, "    limit {} ({})", limit.bytes, limit.source).unwrap();
    write!(
        report,
        "Every clip is already at the bitrate of its slot, so only shorter audio makes the pack smaller.\n\
         Trim changes in {0}, or copy originals from {1} into {0} and trim them there, \
         then build again.",
        ws.changes.display(),
        ws.original.display()
    )
    .unwrap();
    Err(report)
}

fn path(dir: &Path, name: &str) -> String {
    dir.join(name).display().to_string()
}
