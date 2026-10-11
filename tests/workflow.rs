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
use fix_whxm4_voice::workflow::{build_report, change_list, change_names, next_steps, prepare};
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

    let text = next_steps(&ws, "fix-whxm4-voice -d ws");

    let dir = ws.original.parent().unwrap().display().to_string();
    assert!(text.contains(&format!("Next steps, in {dir}:")), "{text}");
    assert!(text.contains("changes/"), "{text}");
    assert!(text.contains("original/"), "{text}");
    assert_eq!(text.matches(&dir).count(), 1, "{text}");
    assert!(text.contains("any audio format"), "{text}");
    assert!(text.contains("empty file"), "{text}");
    assert!(text.contains("`fix-whxm4-voice -d ws` again"), "{text}");
    assert!(text.contains("`fix-whxm4-voice -d ws generate`"), "{text}");
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
        stderr.contains(&format!("`fix-whxm4-voice -d {}` again", dir.display())),
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

#[test]
fn a_language_before_the_command_is_refused_rather_than_ignored() {
    for (flag, command) in [("--lang", "download"), ("--install-lang", "upload")] {
        let out = run(&[flag, "german", command]);

        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(!out.status.success());
        assert!(stderr.contains(&format!("{command} {flag}")), "{stderr}");
    }
}

#[test]
fn change_names_lists_the_files_in_changes() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    assert!(change_names(&ws).unwrap().is_empty());

    fs::write(ws.changes.join("00-mine.wav"), b"mine").unwrap();
    fs::write(ws.changes.join("05-silent.wav"), b"").unwrap();
    fs::write(ws.changes.join(".hidden"), b"").unwrap();

    assert_eq!(change_names(&ws).unwrap(), ["00-mine.wav", "05-silent.wav"]);
}

#[test]
fn default_command_with_changes_offers_an_upload_that_defaults_to_no() {
    let tmp = TempDir::new();
    let dir = tmp.path().join("ws");
    let ws = Workspace::open(&dir, true, |_, _| unreachable!())
        .unwrap()
        .unwrap();
    prepare(&ws, Language::English, fetch).unwrap();
    fs::write(ws.changes.join("00-mine.wav"), b"mine").unwrap();

    // The child's stdin is closed, which answers the offer with nothing.
    let out = run(&["-d", dir.to_str().unwrap(), "-f"]);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(stderr.contains("1 change in"), "{stderr}");
    assert!(stderr.contains("00-mine.wav"), "{stderr}");
    assert!(!stderr.contains("Next steps"), "{stderr}");
    assert!(stderr.contains("Upload 1 change now? [y/N]"), "{stderr}");
    // Declined: how to upload later.
    assert!(
        stderr.contains(&format!("`fix-whxm4-voice -d {}` again", dir.display())),
        "{stderr}"
    );
    assert!(!ws.build.join("pack.bin").exists(), "{stderr}");
}

#[test]
fn default_command_without_changes_offers_no_upload() {
    let tmp = TempDir::new();
    let dir = tmp.path().join("ws");
    let ws = Workspace::open(&dir, true, |_, _| unreachable!())
        .unwrap()
        .unwrap();
    prepare(&ws, Language::English, fetch).unwrap();

    let out = run(&["-d", dir.to_str().unwrap(), "-f"]);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!stderr.contains("[y/N]"), "{stderr}");
    assert!(stderr.contains("Next steps"), "{stderr}");
}

#[test]
fn the_change_list_names_the_changes_folder_and_every_change() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);

    let text = change_list(&ws, &["00-mine.wav".into(), "05-silent.wav".into()]);

    assert!(text.contains(&ws.changes.display().to_string()), "{text}");
    assert!(text.contains("2 changes"), "{text}");
    assert!(text.contains("00-mine.wav"), "{text}");
    assert!(text.contains("05-silent.wav"), "{text}");
}

#[test]
fn a_build_report_names_the_pack_and_every_change() {
    let path = std::path::Path::new("ws/build/pack.bin");

    let text = build_report(path, &["00-mine.wav".into(), "05-silent.wav".into()], true);

    assert!(
        text.starts_with("built and verified ws/build/pack.bin"),
        "{text}"
    );
    assert!(!text.contains("German"), "{text}");
    for expected in ["2 changes", "\n  00-mine.wav", "\n  05-silent.wav"] {
        assert!(text.contains(expected), "{expected} not in:\n{text}");
    }
}

#[test]
fn a_build_report_without_changes_says_it_holds_the_originals_only() {
    let text = build_report(std::path::Path::new("pack.bin"), &[], true);

    assert!(text.contains("no changes"), "{text}");
}

#[test]
fn a_report_of_a_build_used_as_it_is_says_it_was_not_rebuilt() {
    let text = build_report(
        std::path::Path::new("pack.bin"),
        &["00-a.wav".into()],
        false,
    );

    assert!(text.starts_with("using pack.bin"), "{text}");
    assert!(text.contains("not rebuilt"), "{text}");
    assert!(text.contains("00-a.wav"), "{text}");
}

#[test]
fn building_as_english_warns_and_builds_nothing_unless_confirmed() {
    let tmp = TempDir::new();
    let dir = tmp.path().join("ws");
    let ws = Workspace::open(&dir, true, |_, _| unreachable!())
        .unwrap()
        .unwrap();
    prepare(&ws, Language::English, fetch).unwrap();

    // The child's stdin is closed, which answers the question with nothing.
    let out = run(&[
        "-d",
        dir.to_str().unwrap(),
        "build",
        "--install-lang",
        "english",
    ]);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "{stderr}");
    assert!(
        stderr.contains("warning: English is built into the headphones"),
        "{stderr}"
    );
    assert!(stderr.contains("[y/N]"), "{stderr}");
    assert!(!ws.build.join("pack.bin").exists(), "{stderr}");
}

#[test]
fn building_without_an_install_language_builds_german_without_asking() {
    let tmp = TempDir::new();
    let dir = tmp.path().join("ws");
    let ws = Workspace::open(&dir, true, |_, _| unreachable!())
        .unwrap()
        .unwrap();
    prepare(&ws, Language::English, fetch).unwrap();

    let out = run(&["-d", dir.to_str().unwrap(), "build"]);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stderr}");
    assert!(!stderr.contains("[y/N]"), "{stderr}");
    let built = fix_whxm4_voice::build::BuiltPack::read(&ws.build.join("pack.bin")).unwrap();
    assert_eq!(built.language(), Language::German);
}

#[test]
fn default_command_refuses_unmatched_changes_before_offering_the_upload() {
    let tmp = TempDir::new();
    let dir = tmp.path().join("ws");
    let ws = Workspace::open(&dir, true, |_, _| unreachable!())
        .unwrap()
        .unwrap();
    prepare(&ws, Language::English, fetch).unwrap();
    fs::write(ws.changes.join("mine.wav"), b"mine").unwrap();

    let out = run(&["-d", dir.to_str().unwrap(), "-f"]);

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "{stderr}");
    assert!(
        stderr.contains("mine.wav does not start with a slot number"),
        "{stderr}"
    );
    assert!(!stderr.contains("[y/N]"), "{stderr}");
}
