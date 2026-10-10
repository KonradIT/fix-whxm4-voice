use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The folders a workspace holds.
pub const FOLDERS: [&str; 3] = ["original", "changes", "build"];

pub struct Workspace {
    pub original: PathBuf,
    pub changes: PathBuf,
    pub build: PathBuf,
    /// Hidden folder keeping the stock voice pack and manifest, so they are never mistaken for
    /// originals. Created when they are first saved.
    pub stock: PathBuf,
}

impl Workspace {
    pub fn open(
        dir: &Path,
        force: bool,
        confirm: impl FnOnce(&Path, &[String]) -> io::Result<bool>,
    ) -> io::Result<Option<Workspace>> {
        let ws = Workspace {
            original: dir.join(FOLDERS[0]),
            changes: dir.join(FOLDERS[1]),
            build: dir.join(FOLDERS[2]),
            stock: dir.join(".stock"),
        };
        let folders = [&ws.original, &ws.changes, &ws.build];
        if !force && folders.iter().any(|folder| !folder.is_dir()) {
            let others = other_entries(dir)?;
            if !others.is_empty() && !confirm(dir, &others)? {
                return Ok(None);
            }
        }
        for folder in folders {
            fs::create_dir_all(folder)?;
        }
        Ok(Some(ws))
    }
}

/// Non-hidden entries in `dir` other than the workspace folders.
fn other_entries(dir: &Path) -> io::Result<Vec<String>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut names = Vec::new();
    for entry in entries {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if !name.starts_with('.') && !FOLDERS.contains(&name.as_str()) {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}
