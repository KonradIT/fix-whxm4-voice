//! Change resolution: which clip each slot gets, given the files in `changes/`.

use std::fmt;
use std::fs;
use std::io;
use std::path::Path;

use crate::slots::{SLOT_COUNT, clip_file_name};

/// How many original names an unmatched change suggests.
const CLOSEST_COUNT: usize = 3;

/// A file in `changes/`.
#[derive(Clone, Debug)]
pub struct ChangeFile {
    pub name: String,
    /// A zero-byte file, which silences its slot.
    pub empty: bool,
}

/// The clip a slot gets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    /// The slot keeps its original.
    Original,
    /// The slot gets this file from `changes/`, converted to the format of the original.
    Replace(String),
    /// The slot gets a short silent clip.
    Silence,
}

/// A change that cannot be applied.
#[derive(Debug, PartialEq, Eq)]
pub enum Problem {
    /// The change's name starts with no slot number.
    Unmatched {
        change: String,
        closest: Vec<String>,
    },
    /// Several changes start with the same slot number.
    Duplicate {
        original: String,
        changes: Vec<String>,
    },
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Problem::Unmatched { change, closest } => write!(
                f,
                "{change} does not start with a slot number from 00 to {:02}; the closest originals are {}",
                SLOT_COUNT - 1,
                closest.join(", ")
            ),
            Problem::Duplicate { original, changes } => write!(
                f,
                "{} all replace {original}; keep only one",
                changes.join(", ")
            ),
        }
    }
}

/// Lists the files in `changes/`, sorted by name. Hidden entries are left out; anything other than
/// a file is refused, so nothing in `changes/` is silently ignored.
pub fn list(changes: &Path) -> io::Result<Vec<ChangeFile>> {
    let mut files = Vec::new();
    for entry in fs::read_dir(changes)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let metadata = fs::metadata(entry.path())?;
        if !metadata.is_file() {
            return Err(io::Error::other(format!(
                "{} is not a file; changes/ may only hold clips",
                entry.path().display()
            )));
        }
        files.push(ChangeFile {
            name,
            empty: metadata.len() == 0,
        });
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

/// Matches every change to the original with the slot number its name starts with, ignoring the
/// rest of the name, and returns the clip of every slot by slot number. Returns every problem found
/// if any change cannot be applied.
pub fn resolve(changes: &[ChangeFile]) -> Result<Vec<Change>, Vec<Problem>> {
    let originals: Vec<String> = (0..SLOT_COUNT).map(clip_file_name).collect();
    let mut by_slot: Vec<Vec<&ChangeFile>> = vec![Vec::new(); SLOT_COUNT];
    let mut problems = Vec::new();
    for change in changes {
        match slot(&change.name) {
            Some(slot) => by_slot[slot].push(change),
            None => problems.push(Problem::Unmatched {
                change: change.name.clone(),
                closest: closest(stem(&change.name), &originals),
            }),
        }
    }

    let mut resolved = Vec::with_capacity(SLOT_COUNT);
    for (slot, matched) in by_slot.into_iter().enumerate() {
        match matched.as_slice() {
            [] => resolved.push(Change::Original),
            [change] if change.empty => resolved.push(Change::Silence),
            [change] => resolved.push(Change::Replace(change.name.clone())),
            _ => {
                let mut changes: Vec<String> =
                    matched.iter().map(|change| change.name.clone()).collect();
                changes.sort();
                problems.push(Problem::Duplicate {
                    original: originals[slot].clone(),
                    changes,
                });
            }
        }
    }
    if problems.is_empty() {
        Ok(resolved)
    } else {
        Err(problems)
    }
}

/// The slot whose number `name` starts with, or `None` if it starts with no slot number.
pub fn slot(name: &str) -> Option<usize> {
    let digits = name
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(name.len());
    name[..digits]
        .parse()
        .ok()
        .filter(|&slot| slot < SLOT_COUNT)
}

/// The file name without its extension.
fn stem(name: &str) -> &str {
    match name.rfind('.') {
        Some(dot) if dot > 0 => &name[..dot],
        _ => name,
    }
}

/// The original names whose stems are closest to `change_stem`, closest first.
fn closest(change_stem: &str, originals: &[String]) -> Vec<String> {
    let mut by_distance: Vec<(usize, &String)> = originals
        .iter()
        .map(|original| (levenshtein(change_stem, stem(original)), original))
        .collect();
    by_distance.sort();
    by_distance
        .into_iter()
        .take(CLOSEST_COUNT)
        .map(|(_, original)| original.clone())
        .collect()
}

/// Levenshtein distance between `a` and `b`, by character.
fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut current = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let substitute = previous[j] + usize::from(ca != *cb);
            current.push(substitute.min(previous[j + 1] + 1).min(current[j] + 1));
        }
        previous = current;
    }
    previous[b.len()]
}
