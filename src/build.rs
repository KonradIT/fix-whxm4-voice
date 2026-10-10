//! `build`: packs the originals into a voice pack in `build/`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::download::STOCK_PACK;
use crate::invalid;
use crate::pack::{self, Unpacked};
use crate::slots::{SLOT_COUNT, clip_file_name};
use crate::workspace::Workspace;

/// The built voice pack in `build/`, kept after uploading so a failed upload can be investigated.
pub const BUILT_PACK: &str = "pack.bin";

/// Packs the originals into a voice pack with the stock pack's metadata, verifies it by unpacking
/// it again, and writes it to `build/`. Returns the path of the built pack.
pub fn build(ws: &Workspace) -> io::Result<PathBuf> {
    let stock = read_stock(&ws.stock.join(STOCK_PACK))?;
    let stock_image_sha256 = stock.metadata.image_sha256;
    refuse_changes(&ws.changes)?;
    let unpacked = Unpacked {
        clips: read_originals(&ws.original)?,
        ..stock
    };
    let built = pack::pack(&unpacked)?;

    let verified = verify(&built, &unpacked.clips)
        .map_err(|e| invalid(format!("the built voice pack failed verification: {e}")))?;
    if verified.metadata.image_sha256 != stock_image_sha256 {
        return Err(invalid(format!(
            "nothing was changed, but the rebuilt unpacked image differs from the stock one; \
             has a file in {} been edited? Run `download` to restore the originals",
            ws.original.display()
        )));
    }

    let path = ws.build.join(BUILT_PACK);
    fs::write(&path, &built)?;
    Ok(path)
}

fn read_stock(path: &Path) -> io::Result<Unpacked> {
    let stock = fs::read(path).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!(
                "cannot read the stock voice pack {}: {e}; run `download` first",
                path.display()
            ),
        )
    })?;
    pack::unpack(&stock).map_err(|e| {
        invalid(format!(
            "the stock voice pack {} is broken: {e}",
            path.display()
        ))
    })
}

/// Changes are not supported yet, so any are refused rather than silently left out.
fn refuse_changes(changes: &Path) -> io::Result<()> {
    for entry in fs::read_dir(changes)? {
        if !entry?.file_name().to_string_lossy().starts_with('.') {
            return Err(io::Error::other(format!(
                "{} holds files, but changes are not supported yet; empty it to build the unchanged pack",
                changes.display()
            )));
        }
    }
    Ok(())
}

fn read_originals(original: &Path) -> io::Result<Vec<Vec<u8>>> {
    (0..SLOT_COUNT)
        .map(|slot| {
            let path = original.join(clip_file_name(slot));
            fs::read(&path).map_err(|e| {
                io::Error::new(
                    e.kind(),
                    format!(
                        "cannot read {}: {e}; run `download` to restore the originals",
                        path.display()
                    ),
                )
            })
        })
        .collect()
}

/// Unpacks a built pack again, which checks every checksum, the slot table and every clip, and
/// checks that each slot holds the clip packed into it.
fn verify(built: &[u8], clips: &[Vec<u8>]) -> io::Result<Unpacked> {
    let unpacked = pack::unpack(built)?;
    if let Some(slot) = (0..SLOT_COUNT).find(|&slot| unpacked.clips[slot] != clips[slot]) {
        return Err(invalid(format!(
            "slot {slot} does not hold the clip packed into it"
        )));
    }
    Ok(unpacked)
}
