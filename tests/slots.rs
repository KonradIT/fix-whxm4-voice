use fix_whxm4_voice::slots::{SLOT_COUNT, clip_file_name};

#[test]
fn every_slot_has_a_file_name_sorted_by_slot() {
    let names: Vec<String> = (0..SLOT_COUNT).map(clip_file_name).collect();

    assert_eq!(names.len(), 54);
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted);
}

#[test]
fn file_names_carry_slot_number_and_prompt() {
    assert_eq!(clip_file_name(0), "00-power-on.mp3");
    assert_eq!(clip_file_name(7), "07-ambient-sound-off.mp3");
    assert_eq!(clip_file_name(53), "53-second-device-connected.mp3");
}

#[test]
fn slots_of_unknown_purpose_start_with_unknown() {
    assert_eq!(clip_file_name(10), "10-unknown-warning.mp3");
    assert_eq!(clip_file_name(22), "22-unknown-alexa-start.mp3");
    assert_eq!(clip_file_name(46), "46-unknown-3-tones-down-chord.mp3");
}

#[test]
fn file_names_are_lowercase_kebab_case() {
    for name in (0..SLOT_COUNT).map(clip_file_name) {
        let stem = name.strip_suffix(".mp3").unwrap();
        assert!(
            stem.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
            "{name}"
        );
    }
}
