use fix_whxm4_voice::language::Language;

#[test]
fn codes_are_the_ones_the_headphones_use() {
    assert_eq!(Language::English.code(), 0x01);
    assert_eq!(Language::German.code(), 0x03);
    assert_eq!(Language::Finnish.code(), 0x09);
    assert_eq!(Language::Russian.code(), 0x0A);
    assert_eq!(Language::Turkish.code(), 0x10);
    assert_eq!(Language::Chinese.code(), 0xF0);
}

#[test]
fn the_pack_id_is_the_code_in_hex() {
    assert_eq!(Language::English.pack_id(), "VGIDLPB0401");
    assert_eq!(Language::German.pack_id(), "VGIDLPB0403");
    assert_eq!(Language::Korean.pack_id(), "VGIDLPB040F");
    assert_eq!(Language::Turkish.pack_id(), "VGIDLPB0410");
    assert_eq!(Language::Chinese.pack_id(), "VGIDLPB04F0");
}

#[test]
fn a_code_maps_back_to_its_language() {
    assert_eq!(Language::from_code(0x10), Some(Language::Turkish));
    assert_eq!(
        Language::from_code(0x0D),
        Some(Language::BrazilianPortuguese)
    );
    // Simplified Chinese: the headphones know the code, but Sony publishes no pack for it.
    assert_eq!(Language::from_code(0x0C), None);
}

#[test]
fn only_languages_laid_out_like_the_build_can_be_installed() {
    let refused: Vec<Language> = [
        Language::English,
        Language::German,
        Language::Russian,
        Language::Japanese,
        Language::BrazilianPortuguese,
        Language::Korean,
        Language::Chinese,
    ]
    .into_iter()
    .filter(|language| !language.installable())
    .collect();

    assert_eq!(refused, [Language::Japanese, Language::Chinese]);
}
