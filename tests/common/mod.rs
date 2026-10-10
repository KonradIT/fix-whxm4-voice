//! Helpers shared by the integration tests. Each test crate uses only some of them.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use fix_whxm4_voice::pack::{Metadata, Unpacked};
use fix_whxm4_voice::slots::SLOT_COUNT;

/// A fresh empty directory, removed again on drop.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> Self {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("fix-whxm4-voice-test-{}-{n}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        TempDir(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Arbitrary clips of different sizes and contents, one per slot.
pub fn clips() -> Vec<Vec<u8>> {
    (0..SLOT_COUNT)
        .map(|slot| (0..100 + 37 * slot).map(|i| (i * 7 + slot) as u8).collect())
        .collect()
}

/// Metadata the way a stock pack carries it. The sizes and the image hash are deliberately wrong,
/// because packing recomputes them.
pub fn metadata() -> Metadata {
    Metadata {
        compression_type: 2,
        compression_unknown: 1,
        body_offset: 0,
        body_size: 0,
        version: 1,
        header_size: 0,
        unpacked_size: 0,
        sizes_constant: 0x0c38_0000,
        version_string: b"verion_string\0\xff\xff\xff".to_vec(),
        hash_algorithm: 1,
        image_sha256: [0; 32],
    }
}

/// A voice pack taken apart, ready to pack, with stock-like metadata and partition base.
pub fn unpacked(clips: Vec<Vec<u8>>) -> Unpacked {
    Unpacked {
        metadata: metadata(),
        image_version: 1,
        partition_base: 0x8_0000,
        clips,
    }
}
