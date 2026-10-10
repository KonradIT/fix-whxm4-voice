mod common;

use std::fs;

use common::{TempDir, clips, unpacked};
use fix_whxm4_voice::build::{BUILT_PACK, build};
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

fn error(ws: &Workspace) -> String {
    match build(ws) {
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
fn an_edited_original_is_refused() {
    let tmp = TempDir::new();
    let ws = downloaded(&tmp);
    fs::write(ws.original.join(clip_file_name(3)), b"edited").unwrap();

    assert!(error(&ws).contains("stock"));
    assert!(!ws.build.join(BUILT_PACK).exists());
}

#[test]
fn files_in_changes_are_refused_until_changes_are_supported() {
    let tmp = TempDir::new();
    let ws = downloaded(&tmp);
    fs::write(ws.changes.join("00-power-on.mp3"), b"").unwrap();

    assert!(error(&ws).contains("changes"));
}
