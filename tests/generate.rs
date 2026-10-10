mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::TempDir;
use fix_whxm4_voice::generate::generate;
use fix_whxm4_voice::workspace::Workspace;

fn workspace(tmp: &TempDir) -> Workspace {
    Workspace::open(&tmp.path().join("ws"), true, |_, _| unreachable!())
        .unwrap()
        .unwrap()
}

/// A sound script that writes the named clips into the directory it is given.
fn script(tmp: &TempDir, clips: &[&str]) -> PathBuf {
    let path = tmp.path().join("sounds.sh");
    let writes: String = clips
        .iter()
        .map(|clip| format!("echo generated > \"$1/{clip}\"\n"))
        .collect();
    fs::write(
        &path,
        format!("set -eu\nmkdir -p \"$1\"\n{writes}echo done\n"),
    )
    .unwrap();
    path
}

fn changes(ws: &Workspace) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(&ws.changes)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn error(ws: &Workspace, script: &Path, force: bool) -> String {
    match generate(ws, script, force) {
        Ok(_) => panic!("generate succeeded"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn writes_the_generated_clips_into_changes() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    let script = script(&tmp, &["00-power-on.wav", "39-unknown-volume-1.wav"]);

    let generated = generate(&ws, &script, false).unwrap();

    assert_eq!(generated, ["00-power-on.wav", "39-unknown-volume-1.wav"]);
    assert_eq!(changes(&ws), ["00-power-on.wav", "39-unknown-volume-1.wav"]);
    assert_eq!(
        fs::read_to_string(ws.changes.join("00-power-on.wav")).unwrap(),
        "generated\n"
    );
}

#[test]
fn leaves_changes_for_other_slots_alone() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    fs::write(ws.changes.join("07-my-own.mp3"), b"mine").unwrap();
    let script = script(&tmp, &["00-power-on.wav"]);

    generate(&ws, &script, false).unwrap();

    assert_eq!(changes(&ws), ["00-power-on.wav", "07-my-own.mp3"]);
}

#[test]
fn refuses_to_replace_a_change_for_the_same_slot_and_names_it() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    fs::write(ws.changes.join("0-mine.mp3"), b"mine").unwrap();
    fs::write(ws.changes.join("39-unknown-volume-1.wav"), b"mine").unwrap();
    let script = script(
        &tmp,
        &[
            "00-power-on.wav",
            "39-unknown-volume-1.wav",
            "01-power-off.wav",
        ],
    );

    let error = error(&ws, &script, false);

    assert!(error.contains("0-mine.mp3"), "{error}");
    assert!(error.contains("39-unknown-volume-1.wav"), "{error}");
    assert!(error.contains("-f"), "{error}");
    assert_eq!(changes(&ws), ["0-mine.mp3", "39-unknown-volume-1.wav"]);
    assert_eq!(
        fs::read(ws.changes.join("39-unknown-volume-1.wav")).unwrap(),
        b"mine"
    );
}

#[test]
fn force_replaces_changes_for_the_same_slot() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    fs::write(ws.changes.join("0-mine.mp3"), b"mine").unwrap();
    fs::write(ws.changes.join("39-unknown-volume-1.wav"), b"mine").unwrap();
    let script = script(&tmp, &["00-power-on.wav", "39-unknown-volume-1.wav"]);

    generate(&ws, &script, true).unwrap();

    assert_eq!(changes(&ws), ["00-power-on.wav", "39-unknown-volume-1.wav"]);
    assert_eq!(
        fs::read_to_string(ws.changes.join("39-unknown-volume-1.wav")).unwrap(),
        "generated\n"
    );
}

#[test]
fn a_failing_script_leaves_changes_untouched() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    fs::write(ws.changes.join("00-power-on.wav"), b"mine").unwrap();
    let script = tmp.path().join("failing.sh");
    fs::write(&script, "echo partial > \"$1/01-power-off.wav\"\nexit 3\n").unwrap();

    let error = error(&ws, &script, true);

    assert!(error.contains("failing.sh"), "{error}");
    assert_eq!(changes(&ws), ["00-power-on.wav"]);
}

#[test]
fn a_missing_script_is_named() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    let script = tmp.path().join("missing.sh");

    assert!(error(&ws, &script, false).contains("missing.sh"));
}

#[test]
fn a_generated_clip_without_a_slot_number_is_refused() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    let script = script(&tmp, &["power-on.wav"]);

    assert!(error(&ws, &script, false).contains("power-on.wav"));
    assert!(changes(&ws).is_empty());
}

#[test]
fn the_staging_folder_is_cleaned_up() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    let script = script(&tmp, &["00-power-on.wav"]);

    generate(&ws, &script, false).unwrap();

    assert!(fs::read_dir(&ws.build).unwrap().next().is_none());
}
