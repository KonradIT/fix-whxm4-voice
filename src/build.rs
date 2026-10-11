//! `build`: packs the originals, overridden by the changes, into a voice pack in `build/`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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

/// A voice pack that passed the checks `build` makes before writing one: it unpacks with every
/// checksum, the slot table and every clip intact, and it is laid out to install as one of the
/// languages `build` writes, at [`INSTALLED_BASE`]. Only such a pack can be uploaded.
pub struct BuiltPack {
    bytes: Vec<u8>,
    language: Language,
}

impl BuiltPack {
    /// Reads and verifies the pack at `path`, normally [`BUILT_PACK`] in `build/`.
    pub fn read(path: &Path) -> io::Result<Self> {
        let bytes = fs::read(path).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!(
                    "cannot read the built voice pack {}: {e}\nRun `build` first.",
                    path.display()
                ),
            )
        })?;
        Self::verify(bytes).map_err(|e| {
            invalid(format!(
                "{} is not a voice pack as `build` makes it: {e}\nRun `build` again.",
                path.display()
            ))
        })
    }

    /// Verifies `bytes` the way `build` does: a full unpack, then the layout.
    pub fn verify(bytes: Vec<u8>) -> io::Result<Self> {
        let unpacked = pack::unpack(&bytes)?;
        // Only codes `build` writes: how the firmware treats an unknown one is not known (ADR 0003).
        let language = Language::from_code(unpacked.language).ok_or_else(|| {
            invalid(format!(
                "it installs as language {:#x}, which is none of the languages `build` writes",
                unpacked.language
            ))
        })?;
        if unpacked.partition_base != INSTALLED_BASE {
            return Err(invalid(format!(
                "its slot table points at {:#x}, not at {INSTALLED_BASE:#x} where the headphones keep an \
                 installed pack",
                unpacked.partition_base
            )));
        }
        Ok(BuiltPack { bytes, language })
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The language the headphones install the pack as.
    pub fn language(&self) -> Language {
        self.language
    }
}

/// The built voice pack, as [`fresh_with`] finds or makes it.
pub struct Fresh {
    pub pack: BuiltPack,
    /// Whether the pack was built just now.
    pub rebuilt: bool,
}

/// The install language of a build when none is given and there is no earlier build.
pub const DEFAULT_INSTALL: Language = Language::German;

/// The built voice pack, rebuilt first with ffmpeg if it is stale. See [`fresh_with`].
pub fn fresh(ws: &Workspace, install: Option<Language>) -> io::Result<Fresh> {
    fresh_with(ws, install, &Ffmpeg)
}

/// The built voice pack, in `install` if given, or else in the language of the existing build, or
/// else in [`DEFAULT_INSTALL`]. It is rebuilt first with `convert` if there is no build or it is
/// damaged, if it was built for another language, or if `original/`, `changes/` (or a file in them)
/// changed after it was built. Removing a file changes its folder.
pub fn fresh_with(
    ws: &Workspace,
    install: Option<Language>,
    convert: &dyn Convert,
) -> io::Result<Fresh> {
    let path = ws.build.join(BUILT_PACK);
    let built = match fs::metadata(&path) {
        Ok(metadata) => Some(metadata.modified()?),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(e),
    };
    // A build that doesn't verify is simply built again.
    let existing = built.and_then(|_| BuiltPack::read(&path).ok());
    let language = install
        .or(existing.as_ref().map(BuiltPack::language))
        .unwrap_or(DEFAULT_INSTALL);
    if let (Some(built), Some(pack)) = (built, existing)
        && newest_source(ws)? <= built
        && pack.language() == language
    {
        return Ok(Fresh {
            pack,
            rebuilt: false,
        });
    }
    let path = build_with(ws, language, convert)?;
    Ok(Fresh {
        pack: BuiltPack::read(&path)?,
        rebuilt: true,
    })
}

/// When `original/`, `changes/` or a file in them last changed.
fn newest_source(ws: &Workspace) -> io::Result<SystemTime> {
    let mut newest = SystemTime::UNIX_EPOCH;
    for dir in [&ws.original, &ws.changes] {
        newest = newest.max(fs::metadata(dir)?.modified()?);
        for entry in fs::read_dir(dir)? {
            newest = newest.max(fs::metadata(entry?.path())?.modified()?);
        }
    }
    Ok(newest)
}

/// Builds the voice pack, converting changes with ffmpeg. See [`build_with`].
pub fn build(ws: &Workspace, install: Language) -> io::Result<PathBuf> {
    build_with(ws, install, &Ffmpeg)
}

/// Packs a clip per slot into a voice pack with the stock pack's metadata, verifies it by unpacking
/// it again, and writes it to `build/`. Each slot gets its change from `changes/`, converted by
/// `convert` to the format of the stock clip it replaces, or else its original. The pack is laid out
/// like the stock pack of `install`, so the headphones install it as that language. English is
/// built, but the headphones probably play their built-in English instead (ADR 0003). Returns the
/// path of the built pack. Fails with a report if the clips are larger than the [`size::limit`].
pub fn build_with(ws: &Workspace, install: Language, convert: &dyn Convert) -> io::Result<PathBuf> {
    let stock = read_stock(&ws.stock.join(STOCK_PACK))?;
    let stock_language = stock.language;
    let stock_base = stock.partition_base;
    let stock_image_sha256 = stock.metadata.image_sha256;
    let resolved = changes::check(&ws.changes)?;
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
    // Only a stock pack with the same language and layout must come out the same.
    if unchanged
        && stock_language == install.code()
        && stock_base == INSTALLED_BASE
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
                "cannot read the stock voice pack {}: {e}\nRun `download` first.",
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
                "cannot read {}: {e}\nRun `download` to restore the originals.",
                path.display()
            ),
        )
    })?;
    if clip != stock_clip {
        return Err(invalid(format!(
            "{} differs from the stock clip\n\
             Put your version in changes/ instead, and run `download` to restore the originals.",
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
