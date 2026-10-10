//! `generate`: makes changes with the sound script, a bash script that generates clips with ffmpeg.
//!
//! The script stays a script so anyone can read and change it. The Nix package installs it next to
//! the binary and points [`SCRIPT_ENV`] at it; setting that variable runs another copy instead.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::changes;
use crate::workspace::Workspace;

/// Environment variable naming the sound script to run.
pub const SCRIPT_ENV: &str = "FIX_WHXM4_VOICE_GENERATE_SCRIPT";
/// Folder in `build/` the script writes into before its clips are moved into `changes/`.
const STAGING: &str = "generated";

/// The sound script: the one [`SCRIPT_ENV`] names, or else the one in the source tree.
pub fn script() -> PathBuf {
    env::var_os(SCRIPT_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("sounds/generate.sh"))
}

/// Runs `script` with bash and moves the clips it writes into `changes/`. Refuses, before touching
/// `changes/`, if a change for the same slot as a generated clip already exists, unless `force` is
/// set, which replaces those changes. Returns the names of the generated clips.
pub fn generate(ws: &Workspace, script: &Path, force: bool) -> io::Result<Vec<String>> {
    let staging = ws.build.join(STAGING);
    let result = stage(script, &staging).and_then(|generated| {
        replace(ws, &staging, &generated, force)?;
        Ok(generated)
    });
    let _ = fs::remove_dir_all(&staging);
    result
}

/// Runs the script into an empty `staging` folder and returns the names of the clips it wrote.
fn stage(script: &Path, staging: &Path) -> io::Result<Vec<String>> {
    if !script.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("the sound script {} does not exist", script.display()),
        ));
    }
    let _ = fs::remove_dir_all(staging);
    fs::create_dir_all(staging)?;
    let output = Command::new("bash")
        .arg(script)
        .arg(staging)
        .output()
        .map_err(|e| io::Error::new(e.kind(), format!("cannot run bash: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!(
            "the sound script {} failed ({}): {}",
            script.display(),
            output.status,
            stderr.trim()
        )));
    }
    let generated: Vec<String> = changes::list(staging)?
        .into_iter()
        .map(|clip| clip.name)
        .collect();
    if let Some(name) = generated.iter().find(|name| changes::slot(name).is_none()) {
        return Err(io::Error::other(format!(
            "the sound script {} wrote {name}, which does not start with a slot number",
            script.display()
        )));
    }
    Ok(generated)
}

/// Moves the generated clips from `staging` into `changes/`, after removing changes for the same
/// slots if `force` is set, or refusing if it is not.
fn replace(ws: &Workspace, staging: &Path, generated: &[String], force: bool) -> io::Result<()> {
    let slots: BTreeSet<usize> = generated
        .iter()
        .filter_map(|name| changes::slot(name))
        .collect();
    let conflicts: Vec<String> = changes::list(&ws.changes)?
        .into_iter()
        .map(|change| change.name)
        .filter(|name| changes::slot(name).is_some_and(|slot| slots.contains(&slot)))
        .collect();
    if !conflicts.is_empty() && !force {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "{} already holds changes for slots the sound script generates: {}; \
                 run with -f to replace them",
                ws.changes.display(),
                conflicts.join(", ")
            ),
        ));
    }
    for name in &conflicts {
        fs::remove_file(ws.changes.join(name))?;
    }
    for name in generated {
        fs::rename(staging.join(name), ws.changes.join(name))?;
    }
    Ok(())
}
