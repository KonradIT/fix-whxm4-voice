use fix_whxm4_voice::fota::{self, page_crc};

#[test]
fn the_page_crc_of_one_byte_is_its_table_entry_bit_reversed() {
    // CRC-8 table entries 0x01 → 0x31 and 0x08 → 0xB9, as in the APK and race-toolkit.
    assert_eq!(page_crc(&[]), 0x00);
    assert_eq!(page_crc(&[0x01]), 0x31u8.reverse_bits());
    assert_eq!(page_crc(&[0x08]), 0xB9u8.reverse_bits());
}

#[test]
fn a_page_program_carries_storage_one_page_its_crc_address_and_data() {
    let page = [0xA5; 256];

    let packet = fota::page_program(0x02, 0x0812_3400, &page);

    assert_eq!((packet.head, packet.kind, packet.id), (0x15, 0x5A, 0x0402));
    assert_eq!(
        packet.payload[..7],
        [0x02, 0x01, page_crc(&page), 0x00, 0x34, 0x12, 0x08]
    );
    assert_eq!(packet.payload[7..], page);
}

#[test]
fn an_erase_carries_storage_a_4_kb_length_and_the_address() {
    let packet = fota::erase(0x02, 0x0812_3000);

    assert_eq!(
        packet.payload,
        [0x02, 0x00, 0x10, 0x00, 0x00, 0x00, 0x30, 0x12, 0x08]
    );
}
