//! `download`: fetches the stock voice pack and saves its clips as originals.

use std::fs;
use std::io;

use crate::pack;
use crate::slots::clip_file_name;
use crate::source::{self, BaseLanguage};
use crate::workspace::Workspace;

/// The stock manifest, kept in `.stock/` so later builds can reuse it without downloading.
pub const STOCK_MANIFEST: &str = "info.xml";
/// The stock voice pack, kept in `.stock/` so later builds can reuse its metadata.
pub const STOCK_PACK: &str = "pack.bin";

/// Downloads the stock voice pack for `lang`, keeps it with its manifest in `.stock/`, and writes
/// every clip to `original/` as `NN-prompt-name.mp3`. Returns the number of clips written.
pub fn download(ws: &Workspace, lang: BaseLanguage) -> io::Result<usize> {
    let stock = source::fetch(lang)?;
    let unpacked = pack::unpack(&stock.pack)?;
    fs::create_dir_all(&ws.stock)?;
    fs::write(ws.stock.join(STOCK_MANIFEST), &stock.manifest)?;
    fs::write(ws.stock.join(STOCK_PACK), &stock.pack)?;
    for (slot, clip) in unpacked.clips.iter().enumerate() {
        fs::write(ws.original.join(clip_file_name(slot)), clip)?;
    }
    Ok(unpacked.clips.len())
}
