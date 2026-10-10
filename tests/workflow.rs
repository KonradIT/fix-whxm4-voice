mod common;

use std::fs;
use std::io;
use std::process::{Command, Output};

use common::{TempDir, clips, unpacked};
use fix_whxm4_voice::download::STOCK_PACK;
use fix_whxm4_voice::language::Language;
use fix_whxm4_voice::pack::pack;
use fix_whxm4_voice::slots::{SLOT_COUNT, clip_file_name};
use fix_whxm4_voice::source::Stock;
use fix_whxm4_voice::workflow::{next_steps, prepare};
use fix_whxm4_voice::workspace::Workspace;

fn workspace(tmp: &TempDir) -> Workspace {
    Workspace::open(tmp.path(), true, |_, _| unreachable!())
        .unwrap()
        .unwrap()
}

/// Stands in for Sony's update server.
fn fetch(_: Language) -> io::Result<Stock> {
    Ok(Stock {
        manifest: b"manifest".to_vec(),
        pack: pack(&unpacked(clips())).unwrap(),
    })
}

fn never_fetched(_: Language) -> io::Result<Stock> {
    panic!("downloaded again")
}

#[test]
fn first_run_downloads_the_originals() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);

    let downloaded = prepare(&ws, Language::English, fetch).unwrap();

    assert_eq!(downloaded, Some(SLOT_COUNT));
    assert!(ws.stock.join(STOCK_PACK).is_file());
    for (slot, clip) in clips().iter().enumerate() {
        assert_eq!(
            &fs::read(ws.original.join(clip_file_name(slot))).unwrap(),
            clip
        );
    }
}

#[test]
fn rerun_skips_the_download() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    prepare(&ws, Language::English, fetch).unwrap();

    let downloaded = prepare(&ws, Language::English, never_fetched).unwrap();

    assert_eq!(downloaded, None);
}

#[test]
fn a_missing_original_is_downloaded_again() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    prepare(&ws, Language::English, fetch).unwrap();
    fs::remove_file(ws.original.join(clip_file_name(7))).unwrap();

    let downloaded = prepare(&ws, Language::English, fetch).unwrap();

    assert_eq!(downloaded, Some(SLOT_COUNT));
    assert!(ws.original.join(clip_file_name(7)).is_file());
}

#[test]
fn a_missing_stock_pack_is_downloaded_again() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    prepare(&ws, Language::English, fetch).unwrap();
    fs::remove_file(ws.stock.join(STOCK_PACK)).unwrap();

    let downloaded = prepare(&ws, Language::English, fetch).unwrap();

    assert_eq!(downloaded, Some(SLOT_COUNT));
    assert!(ws.stock.join(STOCK_PACK).is_file());
}

#[test]
fn next_steps_explain_the_changes_and_the_upload_command() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);

    let text = next_steps(&ws, "fix-whxm4-voice -d ws upload");

    assert!(text.contains(&ws.changes.display().to_string()), "{text}");
    assert!(text.contains(&ws.original.display().to_string()), "{text}");
    assert!(text.contains("any audio format"), "{text}");
    assert!(text.contains("empty file"), "{text}");
    assert!(text.contains("fix-whxm4-voice -d ws upload"), "{text}");
}

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fix-whxm4-voice"))
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn default_command_with_originals_present_prints_next_steps() {
    let tmp = TempDir::new();
    let dir = tmp.path().join("ws");
    let ws = Workspace::open(&dir, true, |_, _| unreachable!())
        .unwrap()
        .unwrap();
    prepare(&ws, Language::English, fetch).unwrap();

    let out = run(&["-d", dir.to_str().unwrap(), "-f", "--lang", "german"]);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(!stderr.contains("downloading"), "{stderr}");
    assert!(
        stderr.contains(&format!("fix-whxm4-voice -d {} upload", dir.display())),
        "{stderr}"
    );
}

#[test]
fn lang_after_the_download_subcommand_is_parsed() {
    let out = run(&["download", "--lang", "klingon"]);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(stderr.contains("english"), "{stderr}");
}
