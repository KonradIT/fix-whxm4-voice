mod common;

use std::fs;
use std::io;
use std::path::Path;

use common::{TempDir, clips, unpacked};
use fix_whxm4_voice::audio::Convert;
use fix_whxm4_voice::build::{BUILT_PACK, build, build_with};
use fix_whxm4_voice::download::STOCK_PACK;
use fix_whxm4_voice::pack::{pack, unpack};
use fix_whxm4_voice::slots::clip_file_name;
use fix_whxm4_voice::workspace::Workspace;

/// A workspace the way `download` leaves it: the stock pack in `.stock/`, its clips in `original/`.
fn downloaded(tmp: &TempDir) -> Workspace {
    let ws = Workspace::open(tmp.path(), true, |_, _| unreachable!())
        .unwrap()
        .unwrap();
    fs::create_dir_all(&ws.stock).unwrap();
    fs::write(ws.stock.join(STOCK_PACK), pack(&unpacked(clips())).unwrap()).unwrap();
    for (slot, clip) in clips().iter().enumerate() {
        fs::write(ws.original.join(clip_file_name(slot)), clip).unwrap();
    }
    ws
}

/// Stands in for ffmpeg: a converted clip names the change and the size of the stock clip whose
/// format it takes.
struct FakeConvert;

impl Convert for FakeConvert {
    fn convert(&self, change: &Path, stock: &[u8]) -> io::Result<Vec<u8>> {
        let mut clip = fs::read(change)?;
        clip.extend(format!(" like {} bytes", stock.len()).bytes());
        Ok(clip)
    }

    fn silence(&self, stock: &[u8]) -> io::Result<Vec<u8>> {
        Ok(format!("silence like {} bytes", stock.len()).into_bytes())
    }
}

fn built_clips(ws: &Workspace) -> Vec<Vec<u8>> {
    let path = build_with(ws, &FakeConvert).unwrap();
    unpack(&fs::read(path).unwrap()).unwrap().clips
}

fn error(ws: &Workspace) -> String {
    match build_with(ws, &FakeConvert) {
        Ok(_) => panic!("build succeeded"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn builds_the_originals_into_a_pack_in_build() {
    let tmp = TempDir::new();
    let ws = downloaded(&tmp);

    let path = build(&ws).unwrap();

    assert_eq!(path, ws.build.join(BUILT_PACK));
    assert_eq!(unpack(&fs::read(&path).unwrap()).unwrap().clips, clips());
}

#[test]
fn the_unchanged_image_matches_the_stock_image() {
    let tmp = TempDir::new();
    let ws = downloaded(&tmp);

    let built = unpack(&fs::read(build(&ws).unwrap()).unwrap()).unwrap();
    let stock = unpack(&fs::read(ws.stock.join(STOCK_PACK)).unwrap()).unwrap();

    assert_eq!(built.metadata.image_sha256, stock.metadata.image_sha256);
}

#[test]
fn a_change_replaces_its_slot_converted_to_the_stock_format_of_that_slot() {
    let tmp = TempDir::new();
    let ws = downloaded(&tmp);
    fs::write(ws.changes.join("7-ambient-off-8-bit.wav"), b"my clip").unwrap();

    let built = built_clips(&ws);

    let mut expected = clips();
    expected[7] = format!("my clip like {} bytes", clips()[7].len()).into_bytes();
    assert_eq!(built, expected);
}

#[test]
fn an_empty_change_silences_its_slot() {
    let tmp = TempDir::new();
    let ws = downloaded(&tmp);
    fs::write(ws.changes.join("00-power-on.mp3"), b"").unwrap();

    let built = built_clips(&ws);

    assert_eq!(
        built[0],
        format!("silence like {} bytes", clips()[0].len()).into_bytes()
    );
    assert_eq!(built[1..], clips()[1..]);
}

#[test]
fn hidden_files_in_changes_are_left_out() {
    let tmp = TempDir::new();
    let ws = downloaded(&tmp);
    fs::write(ws.changes.join(".DS_Store"), b"junk").unwrap();

    assert_eq!(built_clips(&ws), clips());
}

#[test]
fn an_unmatched_change_fails_and_names_the_closest_originals() {
    let tmp = TempDir::new();
    let ws = downloaded(&tmp);
    fs::write(ws.changes.join("ambient-sound-off.wav"), b"my clip").unwrap();

    let error = error(&ws);

    assert!(error.contains("ambient-sound-off.wav"), "{error}");
    assert!(error.contains("07-ambient-sound-off.mp3"), "{error}");
    assert!(!ws.build.join(BUILT_PACK).exists());
}

#[test]
fn two_changes_for_one_slot_fail() {
    let tmp = TempDir::new();
    let ws = downloaded(&tmp);
    fs::write(ws.changes.join("0-beep.wav"), b"one").unwrap();
    fs::write(ws.changes.join("00-power-on.flac"), b"two").unwrap();

    let error = error(&ws);

    assert!(error.contains("0-beep.wav"), "{error}");
    assert!(error.contains("00-power-on.flac"), "{error}");
}

#[test]
fn a_folder_in_changes_fails() {
    let tmp = TempDir::new();
    let ws = downloaded(&tmp);
    fs::create_dir(ws.changes.join("00-power-on")).unwrap();

    assert!(error(&ws).contains("00-power-on"));
}

#[test]
fn a_missing_stock_pack_asks_for_a_download() {
    let tmp = TempDir::new();
    let ws = downloaded(&tmp);
    fs::remove_file(ws.stock.join(STOCK_PACK)).unwrap();

    assert!(error(&ws).contains("download"));
}

#[test]
fn a_missing_original_is_named() {
    let tmp = TempDir::new();
    let ws = downloaded(&tmp);
    fs::remove_file(ws.original.join(clip_file_name(7))).unwrap();

    assert!(error(&ws).contains("07-ambient-sound-off.mp3"));
}

#[test]
fn an_edited_original_is_refused_and_named() {
    let tmp = TempDir::new();
    let ws = downloaded(&tmp);
    fs::write(ws.original.join(clip_file_name(3)), b"edited").unwrap();
    fs::write(ws.changes.join("00-power-on.mp3"), b"").unwrap();

    let error = error(&ws);

    assert!(error.contains("03-bluetooth-connected.mp3"), "{error}");
    assert!(error.contains("changes"), "{error}");
    assert!(!ws.build.join(BUILT_PACK).exists());
}

#[test]
fn clips_larger_than_the_stock_clips_fail_with_a_report() {
    let tmp = TempDir::new();
    let ws = downloaded(&tmp);
    fs::write(ws.changes.join("00-song.wav"), vec![0; 100_000]).unwrap();

    let error = error(&ws);

    let stock_total: usize = clips().iter().map(Vec::len).sum();
    assert!(error.contains("00-song.wav"), "{error}");
    assert!(error.contains(&format!("limit {stock_total}")), "{error}");
    assert!(error.contains("Trim changes in"), "{error}");
    assert!(!ws.build.join(BUILT_PACK).exists());
}

#[test]
fn a_change_larger_than_its_stock_clip_builds_when_the_total_fits() {
    let tmp = TempDir::new();
    let ws = downloaded(&tmp);
    fs::write(ws.changes.join("00-long.wav"), vec![0; 1000]).unwrap();
    fs::write(ws.changes.join("53-short.wav"), b"short").unwrap();

    let built = built_clips(&ws);

    assert!(built[0].len() > clips()[0].len());
}
