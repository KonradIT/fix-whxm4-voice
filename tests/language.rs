use fix_whxm4_voice::language::Language;

#[test]
fn codes_are_the_ones_the_headphones_use() {
    assert_eq!(Language::English.code(), 0x01);
    assert_eq!(Language::German.code(), 0x03);
    assert_eq!(Language::Finnish.code(), 0x09);
    assert_eq!(Language::Turkish.code(), 0x10);
}

#[test]
fn the_pack_id_is_the_code_in_hex() {
    assert_eq!(Language::English.pack_id(), "VGIDLPB0401");
    assert_eq!(Language::German.pack_id(), "VGIDLPB0403");
    assert_eq!(Language::Turkish.pack_id(), "VGIDLPB0410");
}
