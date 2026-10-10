//! Sony's published keys, kept together in one place.

/// AES-128-ECB key of the manifest body, from the Sound Connect app.
pub const MANIFEST_KEY: [u8; 16] = [
    0x4f, 0xa2, 0x79, 0x99, 0xff, 0xd0, 0x8b, 0x1f, 0xe4, 0xd2, 0x60, 0xd5, 0x7b, 0x6d, 0x3c, 0x17,
];

/// AES-128-CBC key of the voice pack body, from the headphone firmware.
pub const PACK_KEY: [u8; 16] = *b"eibohjeCh6uegahf";

/// AES-128-CBC IV of the voice pack body, from the headphone firmware.
pub const PACK_IV: [u8; 16] = *b"miefeinuShu9eilo";
