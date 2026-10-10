mod common;

use std::process::{Command, Output};

use common::TempDir;

fn completions(shell: &str, dir: &TempDir) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fix-whxm4-voice"))
        .args(["--dir", dir.path().to_str().unwrap(), "completions", shell])
        .output()
        .unwrap()
}

#[test]
fn every_shell_gets_a_script_naming_the_subcommands() {
    for shell in ["bash", "zsh", "fish"] {
        let tmp = TempDir::new();

        let out = completions(shell, &tmp);

        assert!(
            out.status.success(),
            "{shell}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let script = String::from_utf8(out.stdout).unwrap();
        for word in [
            "fix-whxm4-voice",
            "download",
            "build",
            "upload",
            "device-info",
            "dry-run",
        ] {
            assert!(script.contains(word), "{shell} script lacks {word}");
        }
    }
}

#[test]
fn completions_leave_the_workspace_alone() {
    let tmp = TempDir::new();

    assert!(completions("bash", &tmp).status.success());

    assert_eq!(std::fs::read_dir(tmp.path()).unwrap().count(), 0);
}

#[test]
fn unknown_shell_is_refused() {
    let tmp = TempDir::new();

    let out = completions("tcsh", &tmp);

    assert!(!out.status.success());
}
