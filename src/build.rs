//! `build`: packs the originals, overridden by the changes, into a voice pack in `build/`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::audio::{Convert, Ffmpeg};
use crate::changes::{self, Change};
use crate::download::STOCK_PACK;
use crate::invalid;
use crate::language::Language;
use crate::pack::{self, INSTALLED_BASE, Unpacked};
use crate::size;
use crate::slots::{SLOT_COUNT, clip_file_name};
use crate::workspace::Workspace;

/// The built voice pack in `build/`, kept after uploading so a failed upload can be investigated.
pub const BUILT_PACK: &str = "pack.bin";

/// Builds the voice pack, converting changes with ffmpeg. See [`build_with`].
pub fn build(ws: &Workspace, install: Language) -> io::Result<PathBuf> {
    build_with(ws, install, &Ffmpeg)
}

/// Packs a clip per slot into a voice pack with the stock pack's metadata, verifies it by unpacking
/// it again, and writes it to `build/`. Each slot gets its change from `changes/`, converted by
/// `convert` to the format of the stock clip it replaces, or else its original. The pack is laid out
/// like the stock pack of `install`, so the headphones install it as that language. English is
/// refused: the headphones play their built-in English instead of an installed one. Returns the
/// path of the built pack. Fails with a report if the clips are larger than the [`size::limit`].
pub fn build_with(ws: &Workspace, install: Language, convert: &dyn Convert) -> io::Result<PathBuf> {
    if install == Language::English {
        return Err(invalid(
            "a voice pack cannot be installed as English: English is built into the headphones, \
             and they play the built-in English instead; choose another language with --install-lang",
        ));
    }
    let stock = read_stock(&ws.stock.join(STOCK_PACK))?;
    let stock_language = stock.language;
    let stock_image_sha256 = stock.metadata.image_sha256;
    let resolved = changes::resolve(&changes::list(&ws.changes)?).map_err(|problems| {
        let lines: Vec<String> = problems
            .iter()
            .map(|problem| format!("  {problem}"))
            .collect();
        invalid(format!(
            "cannot apply the changes in {}:\n{}",
            ws.changes.display(),
            lines.join("\n")
        ))
    })?;
    let clips: Vec<Vec<u8>> = (0..SLOT_COUNT)
        .map(|slot| {
            let stock_clip = &stock.clips[slot];
            match &resolved[slot] {
                Change::Original => read_original(&ws.original, slot, stock_clip),
                Change::Replace(name) => convert.convert(&ws.changes.join(name), stock_clip),
                Change::Silence => convert.silence(stock_clip),
            }
        })
        .collect::<io::Result<_>>()?;
    size::check(&clips, &resolved, &size::limit(&stock.clips), ws).map_err(invalid)?;
    let unpacked = Unpacked {
        language: install.code(),
        partition_base: INSTALLED_BASE,
        clips,
        ..stock
    };
    let built = pack::pack(&unpacked)?;

    let verified = verify(&built, &unpacked)
        .map_err(|e| invalid(format!("the built voice pack failed verification: {e}")))?;
    let unchanged = resolved.iter().all(|change| *change == Change::Original);
    if unchanged
        && stock_language == install.code()
        && verified.metadata.image_sha256 != stock_image_sha256
    {
        return Err(invalid(
            "nothing was changed, but the rebuilt unpacked image differs from the stock one",
        ));
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

/// Reads the original of `slot`, which must still be the stock clip: edits belong in `changes/`.
fn read_original(original: &Path, slot: usize, stock_clip: &[u8]) -> io::Result<Vec<u8>> {
    let path = original.join(clip_file_name(slot));
    let clip = fs::read(&path).map_err(|e| {
        io::Error::new(
            e.kind(),
            format!(
                "cannot read {}: {e}; run `download` to restore the originals",
                path.display()
            ),
        )
    })?;
    if clip != stock_clip {
        return Err(invalid(format!(
            "{} differs from the stock clip; put your version in changes/ instead, and run \
             `download` to restore the originals",
            path.display()
        )));
    }
    Ok(clip)
}

/// Unpacks a built pack again, which checks every checksum, the slot table and every clip, and
/// checks that it holds the language, partition base and clips packed into it.
fn verify(built: &[u8], packed: &Unpacked) -> io::Result<Unpacked> {
    let unpacked = pack::unpack(built)?;
    if (unpacked.language, unpacked.partition_base) != (packed.language, packed.partition_base) {
        return Err(invalid(format!(
            "the image says language {:#x} at base {:#x}, but language {:#x} at base {:#x} was packed",
            unpacked.language, unpacked.partition_base, packed.language, packed.partition_base
        )));
    }
    if let Some(slot) = (0..SLOT_COUNT).find(|&slot| unpacked.clips[slot] != packed.clips[slot]) {
        return Err(invalid(format!(
            "slot {slot} does not hold the clip packed into it"
        )));
    }
    Ok(unpacked)
}
