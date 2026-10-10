mod common;

use std::path::Path;

use common::TempDir;
use fix_whxm4_voice::workspace::{FOLDERS, Workspace};

fn never_asked(_: &Path, _: &[String]) -> std::io::Result<bool> {
    panic!("asked for confirmation")
}

fn assert_folders(dir: &Path) {
    for name in FOLDERS {
        assert!(dir.join(name).is_dir(), "{name}/ missing");
    }
}

#[test]
fn empty_directory_gets_folders_without_asking() {
    let tmp = TempDir::new();

    let ws = Workspace::open(tmp.path(), false, never_asked)
        .unwrap()
        .unwrap();

    assert_folders(tmp.path());
    assert_eq!(ws.original, tmp.path().join("original"));
    assert_eq!(ws.changes, tmp.path().join("changes"));
    assert_eq!(ws.build, tmp.path().join("build"));
    assert_eq!(ws.stock, tmp.path().join("stock"));
    assert!(ws.stock.is_dir());
}

#[test]
fn missing_directory_is_created() {
    let tmp = TempDir::new();
    let dir = tmp.path().join("foo");

    Workspace::open(&dir, false, never_asked).unwrap().unwrap();

    assert_folders(&dir);
}

#[test]
fn directory_with_other_entries_asks_and_continues_when_accepted() {
    let tmp = TempDir::new();
    std::fs::write(tmp.path().join("notes.txt"), "").unwrap();
    std::fs::create_dir(tmp.path().join("photos")).unwrap();
    let mut asked_about = None;

    let ws = Workspace::open(tmp.path(), false, |_, others| {
        asked_about = Some(others.to_vec());
        Ok(true)
    })
    .unwrap();

    assert!(ws.is_some());
    assert_folders(tmp.path());
    assert_eq!(asked_about.unwrap(), ["notes.txt", "photos"]);
}

#[test]
fn declining_creates_nothing() {
    let tmp = TempDir::new();
    std::fs::write(tmp.path().join("notes.txt"), "").unwrap();

    let ws = Workspace::open(tmp.path(), false, |_, _| Ok(false)).unwrap();

    assert!(ws.is_none());
    for name in FOLDERS {
        assert!(!tmp.path().join(name).exists(), "{name}/ was created");
    }
}

#[test]
fn hidden_entries_never_ask() {
    let tmp = TempDir::new();
    std::fs::write(tmp.path().join(".envrc"), "").unwrap();
    std::fs::create_dir(tmp.path().join(".direnv")).unwrap();
    std::fs::create_dir(tmp.path().join(".git")).unwrap();

    Workspace::open(tmp.path(), false, never_asked)
        .unwrap()
        .unwrap();

    assert_folders(tmp.path());
}

#[test]
fn existing_workspace_never_asks_again() {
    let tmp = TempDir::new();
    std::fs::write(tmp.path().join("notes.txt"), "").unwrap();
    Workspace::open(tmp.path(), false, |_, _| Ok(true))
        .unwrap()
        .unwrap();

    Workspace::open(tmp.path(), false, never_asked)
        .unwrap()
        .unwrap();
}

#[test]
fn workspace_folders_alone_never_ask() {
    let tmp = TempDir::new();
    std::fs::create_dir(tmp.path().join("original")).unwrap();

    Workspace::open(tmp.path(), false, never_asked)
        .unwrap()
        .unwrap();

    assert_folders(tmp.path());
}

#[test]
fn force_skips_the_question() {
    let tmp = TempDir::new();
    std::fs::write(tmp.path().join("notes.txt"), "").unwrap();

    Workspace::open(tmp.path(), true, never_asked)
        .unwrap()
        .unwrap();

    assert_folders(tmp.path());
}
