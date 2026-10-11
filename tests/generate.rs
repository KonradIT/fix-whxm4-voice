mod common;

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use common::{TempDir, clips, unpacked};
use fix_whxm4_voice::download::base_language;
use fix_whxm4_voice::generate::{BATTERY_SCRIPT_ENV, SCRIPT_ENV, Script, generate};
use fix_whxm4_voice::language::Language;
use fix_whxm4_voice::pack::{Unpacked, pack};
use fix_whxm4_voice::source::Stock;
use fix_whxm4_voice::workflow::prepare;
use fix_whxm4_voice::workspace::Workspace;

fn workspace(tmp: &TempDir) -> Workspace {
    Workspace::open(&tmp.path().join("ws"), true, |_, _| unreachable!())
        .unwrap()
        .unwrap()
}

/// A script that writes the named clips into the directory it is given.
fn script(tmp: &TempDir, clips: &[&str]) -> Script {
    script_named(tmp, "sounds.sh", clips)
}

/// Like [`script`], saved as `name`.
fn script_named(tmp: &TempDir, name: &str, clips: &[&str]) -> Script {
    let writes: String = clips
        .iter()
        .map(|clip| format!("echo generated > \"$1/{clip}\"\n"))
        .collect();
    bash(
        tmp,
        name,
        &format!("set -eu\nmkdir -p \"$1\"\n{writes}echo done\n"),
    )
}

/// A script saved as `name` that runs `body`, given no arguments after the staging folder.
fn bash(tmp: &TempDir, name: &str, body: &str) -> Script {
    let path = tmp.path().join(name);
    fs::write(&path, body).unwrap();
    Script {
        path,
        args: Vec::new(),
    }
}

fn changes(ws: &Workspace) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(&ws.changes)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn error(ws: &Workspace, script: Script, force: bool) -> String {
    match generate(ws, &[script], force) {
        Ok(_) => panic!("generate succeeded"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn writes_the_generated_clips_into_changes() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    let script = script(&tmp, &["00-power-on.wav", "39-unknown-volume-1.wav"]);

    let generated = generate(&ws, &[script], false).unwrap();

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

    generate(&ws, &[script], false).unwrap();

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

    let error = error(&ws, script, false);

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

    generate(&ws, &[script], true).unwrap();

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
    let script = bash(
        &tmp,
        "failing.sh",
        "echo partial > \"$1/01-power-off.wav\"\nexit 3\n",
    );

    let error = error(&ws, script, true);

    assert!(error.contains("failing.sh"), "{error}");
    assert_eq!(changes(&ws), ["00-power-on.wav"]);
}

#[test]
fn a_missing_script_is_named() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    let script = Script {
        path: tmp.path().join("missing.sh"),
        args: Vec::new(),
    };

    assert!(error(&ws, script, false).contains("missing.sh"));
}

#[test]
fn the_clips_of_every_script_go_into_changes() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    let sounds = script(&tmp, &["00-power-on.wav"]);
    let battery = script_named(&tmp, "battery.sh", &["16-battery-70.wav"]);

    let generated = generate(&ws, &[sounds, battery], false).unwrap();

    assert_eq!(generated, ["00-power-on.wav", "16-battery-70.wav"]);
    assert_eq!(changes(&ws), ["00-power-on.wav", "16-battery-70.wav"]);
}

#[test]
fn a_script_gets_its_arguments_after_the_staging_folder() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    let mut script = bash(
        &tmp,
        "battery.sh",
        "echo \"$2\" > \"$1/16-battery-70.wav\"\n",
    );
    script.args.push("the originals".into());

    generate(&ws, &[script], false).unwrap();

    assert_eq!(
        fs::read_to_string(ws.changes.join("16-battery-70.wav")).unwrap(),
        "the originals\n"
    );
}

#[test]
fn the_battery_script_reads_the_originals_of_the_workspace() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);

    let script = Script::battery(&ws);

    assert_eq!(script.args, [ws.original.clone().into_os_string()]);
    assert!(
        script.path.ends_with("battery.sh"),
        "{}",
        script.path.display()
    );
}

#[test]
fn a_failing_later_script_leaves_changes_untouched() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    let sounds = script(&tmp, &["00-power-on.wav"]);
    let failing = bash(&tmp, "battery.sh", "exit 1\n");

    let error = match generate(&ws, &[sounds, failing], false) {
        Ok(_) => panic!("generate succeeded"),
        Err(e) => e.to_string(),
    };

    assert!(error.contains("battery.sh"), "{error}");
    assert!(changes(&ws).is_empty());
}

#[test]
fn a_change_for_a_slot_of_a_later_script_is_refused() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    fs::write(ws.changes.join("16-mine.mp3"), b"mine").unwrap();
    let sounds = script(&tmp, &["00-power-on.wav"]);
    let battery = script_named(&tmp, "battery.sh", &["16-battery-70.wav"]);

    let error = match generate(&ws, &[sounds, battery], false) {
        Ok(_) => panic!("generate succeeded"),
        Err(e) => e.to_string(),
    };

    assert!(error.contains("16-mine.mp3"), "{error}");
    assert_eq!(changes(&ws), ["16-mine.mp3"]);
}

#[test]
fn a_generated_clip_without_a_slot_number_is_refused() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    let script = script(&tmp, &["power-on.wav"]);

    assert!(error(&ws, script, false).contains("power-on.wav"));
    assert!(changes(&ws).is_empty());
}

#[test]
fn the_staging_folder_is_cleaned_up() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    let script = script(&tmp, &["00-power-on.wav"]);

    generate(&ws, &[script], false).unwrap();

    assert!(fs::read_dir(&ws.build).unwrap().next().is_none());
}

/// Downloads stand-in originals into `ws`, from a stock pack in `language`.
fn download(ws: &Workspace, language: Language) {
    let fetch = |_| -> io::Result<Stock> {
        Ok(Stock {
            manifest: b"manifest".to_vec(),
            pack: pack(&Unpacked {
                language: language.code(),
                ..unpacked(clips())
            })
            .unwrap(),
        })
    };
    prepare(ws, Language::English, fetch).unwrap();
}

#[test]
fn the_base_language_is_the_language_of_the_stock_pack() {
    let tmp = TempDir::new();
    let ws = workspace(&tmp);
    assert_eq!(base_language(&ws).unwrap(), None);

    download(&ws, Language::German);

    assert_eq!(base_language(&ws).unwrap(), Some(Language::German));
}

/// Runs `generate` in the workspace at `dir` with `args`, stand-in scripts writing a tone clip and a
/// battery clip, and `input` on stdin.
fn run_generate(tmp: &TempDir, dir: &Path, args: &[&str], input: &str) -> Output {
    let sounds = script(tmp, &["00-power-on.wav"]);
    let battery = script_named(tmp, "battery.sh", &["16-battery-70.wav"]);
    let mut child = Command::new(env!("CARGO_BIN_EXE_fix-whxm4-voice"))
        .args(["-d", dir.to_str().unwrap(), "-f", "generate"])
        .args(args)
        .env(SCRIPT_ENV, sounds.path)
        .env(BATTERY_SCRIPT_ENV, battery.path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

/// Sets up a workspace with originals in `language`, or none, and returns it with its directory.
fn originals_in(tmp: &TempDir, language: Option<Language>) -> (Workspace, PathBuf) {
    let dir = tmp.path().join("ws");
    let ws = workspace(tmp);
    if let Some(language) = language {
        download(&ws, language);
    }
    (ws, dir)
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn battery_extracts_the_battery_clips_without_asking() {
    let tmp = TempDir::new();
    let (ws, dir) = originals_in(&tmp, Some(Language::English));

    let out = run_generate(&tmp, &dir, &["--battery"], "");

    let stderr = stderr(&out);
    assert!(out.status.success(), "{stderr}");
    assert!(!stderr.contains("[y/N]"), "{stderr}");
    assert_eq!(changes(&ws), ["00-power-on.wav", "16-battery-70.wav"]);
}

#[test]
fn no_battery_generates_the_tones_only_without_asking() {
    let tmp = TempDir::new();
    let (ws, dir) = originals_in(&tmp, Some(Language::English));

    let out = run_generate(&tmp, &dir, &["--no-battery"], "");

    let stderr = stderr(&out);
    assert!(out.status.success(), "{stderr}");
    assert!(!stderr.contains("[y/N]"), "{stderr}");
    assert_eq!(changes(&ws), ["00-power-on.wav"]);
}

#[test]
fn battery_and_no_battery_conflict() {
    let tmp = TempDir::new();
    let (ws, dir) = originals_in(&tmp, Some(Language::English));

    let out = run_generate(&tmp, &dir, &["--battery", "--no-battery"], "");

    assert!(!out.status.success());
    assert!(changes(&ws).is_empty());
}

#[test]
fn with_english_originals_it_asks_and_defaults_to_no() {
    let tmp = TempDir::new();
    let (ws, dir) = originals_in(&tmp, Some(Language::English));

    let out = run_generate(&tmp, &dir, &[], "\n");

    let stderr = stderr(&out);
    assert!(out.status.success(), "{stderr}");
    assert!(stderr.contains("English originals"), "{stderr}");
    assert!(stderr.contains("[y/N]"), "{stderr}");
    assert_eq!(changes(&ws), ["00-power-on.wav"]);
}

#[test]
fn the_tones_are_generated_before_the_battery_question() {
    let tmp = TempDir::new();
    let (_, dir) = originals_in(&tmp, Some(Language::English));

    let out = run_generate(&tmp, &dir, &[], "\n");

    let stderr = stderr(&out);
    let tones = stderr.find("00-power-on.wav").expect(&stderr);
    let question = stderr.find("[y/N]").expect(&stderr);
    assert!(tones < question, "{stderr}");
}

#[test]
fn with_english_originals_yes_extracts_the_battery_clips() {
    let tmp = TempDir::new();
    let (ws, dir) = originals_in(&tmp, Some(Language::English));

    let out = run_generate(&tmp, &dir, &[], "y\n");

    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(changes(&ws), ["00-power-on.wav", "16-battery-70.wav"]);
}

#[test]
fn with_other_originals_it_says_english_only_and_skips_even_with_battery() {
    for args in [&[][..], &["--battery"]] {
        let tmp = TempDir::new();
        let (ws, dir) = originals_in(&tmp, Some(Language::German));

        let out = run_generate(&tmp, &dir, args, "y\n");

        let stderr = stderr(&out);
        assert!(out.status.success(), "{stderr}");
        assert!(!stderr.contains("[y/N]"), "{stderr}");
        assert!(stderr.contains("English"), "{stderr}");
        assert!(stderr.contains("german"), "{stderr}");
        assert_eq!(changes(&ws), ["00-power-on.wav"]);
    }
}

#[test]
fn with_other_originals_no_battery_says_nothing_about_them() {
    let tmp = TempDir::new();
    let (_, dir) = originals_in(&tmp, Some(Language::German));

    let out = run_generate(&tmp, &dir, &["--no-battery"], "");

    let stderr = stderr(&out);
    assert!(out.status.success(), "{stderr}");
    assert!(!stderr.contains("battery"), "{stderr}");
}

#[test]
fn without_originals_it_offers_to_download_the_english_ones() {
    let tmp = TempDir::new();
    let (ws, dir) = originals_in(&tmp, None);

    // Declined, so nothing is downloaded.
    let out = run_generate(&tmp, &dir, &[], "\n");

    let stderr = stderr(&out);
    assert!(out.status.success(), "{stderr}");
    assert!(stderr.contains("no originals"), "{stderr}");
    assert!(stderr.contains("Download"), "{stderr}");
    assert!(!stderr.contains("downloading"), "{stderr}");
    assert_eq!(changes(&ws), ["00-power-on.wav"]);
    assert!(fs::read_dir(&ws.original).unwrap().next().is_none());
}

#[test]
fn without_originals_and_another_lang_it_skips_without_asking() {
    let tmp = TempDir::new();
    let (ws, dir) = originals_in(&tmp, None);

    let out = run_generate(&tmp, &dir, &["--lang", "german"], "y\n");

    let stderr = stderr(&out);
    assert!(out.status.success(), "{stderr}");
    assert!(!stderr.contains("[y/N]"), "{stderr}");
    assert!(stderr.contains("german"), "{stderr}");
    assert_eq!(changes(&ws), ["00-power-on.wav"]);
}
