use fix_whxm4_voice::changes::{Change, ChangeFile, Problem, resolve};
use fix_whxm4_voice::slots::SLOT_COUNT;

fn file(name: &str) -> ChangeFile {
    ChangeFile {
        name: name.to_string(),
        empty: false,
    }
}

fn empty(name: &str) -> ChangeFile {
    ChangeFile {
        name: name.to_string(),
        empty: true,
    }
}

fn problems(changes: &[ChangeFile]) -> Vec<Problem> {
    match resolve(changes) {
        Ok(_) => panic!("changes were accepted"),
        Err(problems) => problems,
    }
}

fn replace(name: &str) -> Change {
    Change::Replace(name.to_string())
}

#[test]
fn without_changes_every_slot_keeps_its_original() {
    let resolved = resolve(&[]).unwrap();

    assert_eq!(resolved, vec![Change::Original; SLOT_COUNT]);
}

#[test]
fn a_change_replaces_the_original_with_the_same_slot_number() {
    let resolved = resolve(&[file("07-ambient-sound-off.mp3")]).unwrap();

    assert_eq!(resolved[7], replace("07-ambient-sound-off.mp3"));
    assert_eq!(resolved[6], Change::Original);
    assert_eq!(resolved[8], Change::Original);
}

#[test]
fn matching_ignores_everything_after_the_slot_number() {
    let resolved = resolve(&[
        file("7-ambient-off-8-bit.wav"),
        file("053"),
        file("10.flac"),
        file("12ring.ogg"),
    ])
    .unwrap();

    assert_eq!(resolved[7], replace("7-ambient-off-8-bit.wav"));
    assert_eq!(resolved[53], replace("053"));
    assert_eq!(resolved[10], replace("10.flac"));
    assert_eq!(resolved[12], replace("12ring.ogg"));
}

#[test]
fn an_empty_change_silences_the_slot() {
    let resolved = resolve(&[empty("00-power-on.mp3")]).unwrap();

    assert_eq!(resolved[0], Change::Silence);
}

#[test]
fn two_changes_for_the_same_slot_are_a_problem() {
    let problems = problems(&[file("7-beep.wav"), empty("07-ambient-sound-off.mp3")]);

    assert_eq!(
        problems,
        vec![Problem::Duplicate {
            original: "07-ambient-sound-off.mp3".to_string(),
            changes: vec![
                "07-ambient-sound-off.mp3".to_string(),
                "7-beep.wav".to_string()
            ],
        }]
    );
}

#[test]
fn a_change_without_a_slot_number_lists_the_closest_original_names() {
    let problems = problems(&[file("ambient-sound-off.wav")]);

    let [Problem::Unmatched { change, closest }] = problems.as_slice() else {
        panic!("expected one unmatched change, got {problems:?}");
    };
    assert_eq!(change, "ambient-sound-off.wav");
    assert_eq!(closest[0], "07-ambient-sound-off.mp3");
    assert!(closest.len() > 1 && closest.len() <= 3, "{closest:?}");
}

#[test]
fn a_change_with_a_slot_number_past_the_last_slot_is_unmatched() {
    let problems = problems(&[
        file("54-power-on.mp3"),
        file("99999999999999999999999-x.wav"),
    ]);

    assert!(
        matches!(
            problems.as_slice(),
            [Problem::Unmatched { .. }, Problem::Unmatched { .. }]
        ),
        "{problems:?}"
    );
}

#[test]
fn every_problem_is_reported_at_once() {
    let problems = problems(&[
        file("power-on.mp3"),
        file("0-a.wav"),
        file("00-b.flac"),
        file("01-power-of.mp3"),
    ]);

    assert_eq!(problems.len(), 2, "{problems:?}");
}

#[test]
fn a_problem_reads_as_a_sentence_naming_the_files() {
    let unmatched = problems(&[file("ambient-sound-off.wav")])
        .remove(0)
        .to_string();
    let duplicate = problems(&[file("0-beep.wav"), file("00-power-on.mp3")])
        .remove(0)
        .to_string();

    assert!(unmatched.contains("ambient-sound-off.wav"), "{unmatched}");
    assert!(
        unmatched.contains("07-ambient-sound-off.mp3"),
        "{unmatched}"
    );
    assert!(duplicate.contains("0-beep.wav"), "{duplicate}");
    assert!(duplicate.contains("00-power-on.mp3"), "{duplicate}");
}
