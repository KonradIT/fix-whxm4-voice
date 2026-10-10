//! `download`: fetches the stock voice pack and saves its clips as originals.

use std::fs;
use std::io;

use crate::language::Language;
use crate::pack;
use crate::slots::{SLOT_COUNT, clip_file_name};
use crate::source::{self, Stock};
use crate::workspace::Workspace;

/// The stock manifest, kept in `stock/` so later builds can reuse it without downloading.
pub const STOCK_MANIFEST: &str = "info.xml";
/// The stock voice pack, kept in `stock/` so later builds can reuse its metadata.
pub const STOCK_PACK: &str = "pack.bin";

/// Downloads the stock voice pack for `lang`, keeps it with its manifest in `stock/`, and writes
/// every clip to `original/` as `NN-prompt-name.mp3`. Returns the number of clips written.
pub fn download(ws: &Workspace, lang: Language) -> io::Result<usize> {
    download_with(ws, lang, source::fetch)
}

/// Whether the stock voice pack and every original are present, as [`download`] leaves them.
pub fn downloaded(ws: &Workspace) -> bool {
    ws.stock.join(STOCK_PACK).is_file()
        && (0..SLOT_COUNT).all(|slot| ws.original.join(clip_file_name(slot)).is_file())
}

/// Like [`download`], getting the stock manifest and voice pack from `fetch`.
pub fn download_with(
    ws: &Workspace,
    lang: Language,
    fetch: impl FnOnce(Language) -> io::Result<Stock>,
) -> io::Result<usize> {
    let stock = fetch(lang)?;
    let unpacked = pack::unpack(&stock.pack)?;
    fs::create_dir_all(&ws.stock)?;
    fs::write(ws.stock.join(STOCK_MANIFEST), &stock.manifest)?;
    fs::write(ws.stock.join(STOCK_PACK), &stock.pack)?;
    for (slot, clip) in unpacked.clips.iter().enumerate() {
        fs::write(ws.original.join(clip_file_name(slot)), clip)?;
    }
    Ok(unpacked.clips.len())
}
