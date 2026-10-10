mod common;

use common::{clips, unpacked};
use fix_whxm4_voice::pack::{IMAGE_SIZE, pack, unpack};
use fix_whxm4_voice::slots::SLOT_COUNT;
use sha2::{Digest, Sha256};

/// Size of the slot table at the start of the unpacked image, which the clips share 1 MiB with.
const TABLE_SIZE: usize = 8 + 8 * SLOT_COUNT;

/// The arbitrary clips, with slot 0 grown until clips and slot table take `image_size` bytes.
fn clips_filling(image_size: usize) -> Vec<Vec<u8>> {
    let mut clips = clips();
    let used: usize = TABLE_SIZE + clips.iter().map(Vec::len).sum::<usize>();
    let len = clips[0].len() + image_size - used;
    clips[0].resize(len, 0x55);
    clips
}

/// Recomputes the SHA-256 at offset 0, so a tampered pack gets past the outer check.
fn resign(pack: &mut [u8]) {
    let sha = Sha256::digest(&pack[0x100..]);
    pack[..32].copy_from_slice(&sha);
}

fn flip(pack: &[u8], pos: usize) -> Vec<u8> {
    let mut pack = pack.to_vec();
    pack[pos] ^= 0x01;
    pack
}

fn error(pack: &[u8]) -> String {
    match unpack(pack) {
        Ok(_) => panic!("tampered pack was accepted"),
        Err(e) => e.to_string(),
    }
}

#[test]
fn a_packed_pack_unpacks_to_the_same_clips() {
    let clips = clips();
    let unpacked = unpack(&pack(&unpacked(clips.clone())).unwrap()).unwrap();

    assert_eq!(unpacked.clips, clips);
    assert_eq!(unpacked.image_version, 1);
    assert_eq!(unpacked.partition_base, 0x8_0000);
}

#[test]
fn packing_keeps_the_stock_metadata_and_recomputes_sizes_and_hash() {
    let pack = pack(&unpacked(clips())).unwrap();
    let metadata = unpack(&pack).unwrap().metadata;

    assert_eq!(metadata.compression_type, 2);
    assert_eq!(metadata.compression_unknown, 1);
    assert_eq!(metadata.version, 1);
    assert_eq!(metadata.sizes_constant, 0x0c38_0000);
    assert_eq!(metadata.version_string, b"verion_string\0\xff\xff\xff");
    assert_eq!(metadata.hash_algorithm, 1);
    assert_eq!(metadata.body_offset, 0x1000);
    assert_eq!(metadata.header_size, 0x1000);
    assert_eq!(metadata.body_size as usize, pack.len() - 0x1000);
    assert_eq!(metadata.unpacked_size as usize, IMAGE_SIZE);
    assert_ne!(metadata.image_sha256, [0; 32]);
}

#[test]
fn the_body_is_whole_aes_blocks() {
    let pack = pack(&unpacked(clips())).unwrap();

    assert_eq!((pack.len() - 0x1000) % 16, 0);
}

#[test]
fn repacking_an_unpacked_pack_reproduces_it_byte_for_byte() {
    let first = pack(&unpacked(clips())).unwrap();
    let second = pack(&unpack(&first).unwrap()).unwrap();

    assert!(first == second, "repacked pack differs");
}

#[test]
fn clips_filling_exactly_one_mib_fit() {
    let clips = clips_filling(IMAGE_SIZE);

    let unpacked = unpack(&pack(&unpacked(clips.clone())).unwrap()).unwrap();

    assert_eq!(unpacked.clips, clips);
}

#[test]
fn an_image_over_one_mib_is_rejected() {
    let clips = clips_filling(IMAGE_SIZE + 1);

    let error = pack(&unpacked(clips)).expect_err("oversized image was packed");

    assert!(error.to_string().contains("1 MiB"), "{error}");
}

#[test]
fn a_wrong_number_of_clips_is_rejected() {
    let mut clips = clips();
    clips.pop();

    assert!(pack(&unpacked(clips)).is_err());
}

#[test]
fn tampering_with_the_outer_sha256_is_detected() {
    let pack = pack(&unpacked(clips())).unwrap();

    assert!(error(&flip(&pack, 5)).contains("offset 0"));
}

#[test]
fn tampering_with_the_metadata_is_detected() {
    let pack = pack(&unpacked(clips())).unwrap();

    for pos in [0x100, 0x110, 0x200, 0xfff] {
        assert!(error(&flip(&pack, pos)).contains("offset 0"), "{pos:#x}");
    }
}

#[test]
fn tampering_with_the_body_is_detected() {
    let pack = pack(&unpacked(clips())).unwrap();

    for pos in [0x1000, 0x1000 + (pack.len() - 0x1000) / 2, pack.len() - 1] {
        assert!(error(&flip(&pack, pos)).contains("offset 0"), "{pos:#x}");
    }
}

#[test]
fn a_body_that_no_longer_matches_the_image_sha256_is_detected() {
    let pack = pack(&unpacked(clips())).unwrap();
    // A byte inside the compressed stream, past the 13-byte LZMA header.
    let mut tampered = flip(&pack, 0x1000 + (pack.len() - 0x1000) / 2);
    resign(&mut tampered);

    assert!(unpack(&tampered).is_err());
}

#[test]
fn a_changed_image_sha256_in_the_metadata_is_detected() {
    let mut tampered = pack(&unpacked(clips())).unwrap();
    let hash = tampered[0x100..0x1000]
        .windows(4)
        .position(|w| w == [0x14, 0x00, 0x24, 0x00])
        .expect("tag 0x0014 not found")
        + 0x100
        + 8;
    tampered[hash] ^= 0x01;
    resign(&mut tampered);

    assert!(error(&tampered).contains("SHA-256 in the metadata"));
}
