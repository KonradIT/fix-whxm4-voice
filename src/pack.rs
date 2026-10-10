//! The pack codec: Sony's voice pack format.
//!
//! - 0x0000: SHA-256 of everything from 0x100 to the end.
//! - 0x0100: metadata block of tag-length entries (u16 tag, u16 length, little-endian), padded with
//!   0xFF to the body:
//!   - 0x0011: compression type (u8, 2 = LZMA), an unknown u8 (1), body offset (u32), body size (u32);
//!   - 0x0012: version, header size, unpacked size, a constant (u32 each);
//!   - 0x0013: version string (Sony spells the field `verion_string`);
//!   - 0x0014: hash algorithm (u32), SHA-256 of the unpacked image.
//! - 0x1000: body, AES-128-CBC over a 13-byte LZMA1 header and raw LZMA1 stream, padded with 0xFF.
//!
//! The unpacked image is a u32 version, a u32 slot count, a table of (u32 size, u32 offset) per
//! slot, then the clips back to back, zero-padded to 1 MiB. Offsets are absolute in the voice
//! partition; the partition base is the first offset minus the end of the table.

use std::io;

use aes::Aes128;
use aes::cipher::block_padding::NoPadding;
use aes::cipher::{BlockModeDecrypt, KeyIvInit};
use sha2::{Digest, Sha256};

use crate::invalid;
use crate::keys::{PACK_IV, PACK_KEY};
use crate::slots::SLOT_COUNT;

/// Offset of the metadata block, and start of the region the outer SHA-256 covers.
const METADATA_OFFSET: usize = 0x100;
/// Offset of the body; everything before it is the header.
const BODY_OFFSET: usize = 0x1000;
/// Size of the unpacked image.
pub const IMAGE_SIZE: usize = 0x10_0000;
/// Compression type of an LZMA1 body.
const LZMA: u8 = 2;

const TAG_COMPRESSION: u16 = 0x0011;
const TAG_SIZES: u16 = 0x0012;
const TAG_VERSION_STRING: u16 = 0x0013;
const TAG_IMAGE_HASH: u16 = 0x0014;
const TAG_END: u16 = 0xFFFF;

/// A voice pack taken apart.
pub struct Unpacked {
    pub metadata: Metadata,
    /// Version field at the start of the unpacked image.
    pub image_version: u32,
    /// Address of the unpacked image in the voice partition.
    pub partition_base: u32,
    /// The clip of every slot, by slot number.
    pub clips: Vec<Vec<u8>>,
}

/// The metadata block at 0x100.
pub struct Metadata {
    pub compression_type: u8,
    /// Second byte of tag 0x0011; meaning unknown, 1 in stock packs.
    pub compression_unknown: u8,
    pub body_offset: u32,
    pub body_size: u32,
    pub version: u32,
    pub header_size: u32,
    pub unpacked_size: u32,
    /// Fourth field of tag 0x0012; meaning unknown, 0x0c380000 in stock packs.
    pub sizes_constant: u32,
    /// Raw value of tag 0x0013, including its NUL and 0xFF filler.
    pub version_string: Vec<u8>,
    pub hash_algorithm: u32,
    pub image_sha256: [u8; 32],
}

/// Takes a voice pack apart, checking every checksum and size along the way.
pub fn unpack(pack: &[u8]) -> io::Result<Unpacked> {
    if pack.len() <= BODY_OFFSET {
        return Err(invalid(format!("voice pack is only {} bytes", pack.len())));
    }
    if Sha256::digest(&pack[METADATA_OFFSET..])[..] != pack[..32] {
        return Err(invalid(
            "voice pack SHA-256 at offset 0 does not match its contents",
        ));
    }
    let metadata = read_metadata(&pack[METADATA_OFFSET..BODY_OFFSET])?;
    if metadata.compression_type != LZMA {
        return Err(invalid(format!(
            "unknown compression type {}",
            metadata.compression_type
        )));
    }
    if metadata.body_offset as usize != BODY_OFFSET
        || metadata.body_offset as usize + metadata.body_size as usize != pack.len()
    {
        return Err(invalid(format!(
            "metadata puts the body at {:#x}+{:#x}, but the pack is {:#x} bytes",
            metadata.body_offset,
            metadata.body_size,
            pack.len()
        )));
    }
    if metadata.unpacked_size as usize != IMAGE_SIZE {
        return Err(invalid(format!(
            "metadata says the unpacked image is {:#x} bytes",
            metadata.unpacked_size
        )));
    }

    let image = decompress(&decrypt(&pack[BODY_OFFSET..])?)?;
    if image.len() != IMAGE_SIZE {
        return Err(invalid(format!(
            "unpacked image is {:#x} bytes, expected {IMAGE_SIZE:#x}",
            image.len()
        )));
    }
    if Sha256::digest(&image)[..] != metadata.image_sha256 {
        return Err(invalid(
            "unpacked image does not match the SHA-256 in the metadata",
        ));
    }
    let (image_version, partition_base, clips) = read_image(&image)?;

    Ok(Unpacked {
        metadata,
        image_version,
        partition_base,
        clips,
    })
}

fn read_metadata(block: &[u8]) -> io::Result<Metadata> {
    let mut compression = None;
    let mut sizes = None;
    let mut version_string = None;
    let mut image_hash = None;

    let mut pos = 0;
    while pos + 4 <= block.len() {
        let tag = u16_at(block, pos);
        if tag == TAG_END {
            break;
        }
        let len = u16_at(block, pos + 2) as usize;
        let value = block
            .get(pos + 4..pos + 4 + len)
            .ok_or_else(|| invalid(format!("metadata tag {tag:#06x} runs past the header")))?;
        match tag {
            TAG_COMPRESSION => compression = Some(value),
            TAG_SIZES => sizes = Some(value),
            TAG_VERSION_STRING => version_string = Some(value),
            TAG_IMAGE_HASH => image_hash = Some(value),
            _ => return Err(invalid(format!("unknown metadata tag {tag:#06x}"))),
        }
        pos += 4 + len;
    }

    let compression = sized_tag(compression, TAG_COMPRESSION, 10)?;
    let sizes = sized_tag(sizes, TAG_SIZES, 16)?;
    let image_hash = sized_tag(image_hash, TAG_IMAGE_HASH, 36)?;
    let version_string = version_string.ok_or_else(|| invalid("metadata tag 0x0013 is missing"))?;

    Ok(Metadata {
        compression_type: compression[0],
        compression_unknown: compression[1],
        body_offset: u32_at(compression, 2),
        body_size: u32_at(compression, 6),
        version: u32_at(sizes, 0),
        header_size: u32_at(sizes, 4),
        unpacked_size: u32_at(sizes, 8),
        sizes_constant: u32_at(sizes, 12),
        version_string: version_string.to_vec(),
        hash_algorithm: u32_at(image_hash, 0),
        image_sha256: image_hash[4..].try_into().unwrap(),
    })
}

/// The value of a metadata tag that must be present with a fixed length.
fn sized_tag(value: Option<&[u8]>, tag: u16, len: usize) -> io::Result<&[u8]> {
    match value {
        Some(value) if value.len() == len => Ok(value),
        Some(value) => Err(invalid(format!(
            "metadata tag {tag:#06x} is {} bytes, expected {len}",
            value.len()
        ))),
        None => Err(invalid(format!("metadata tag {tag:#06x} is missing"))),
    }
}

fn decrypt(body: &[u8]) -> io::Result<Vec<u8>> {
    let mut plain = body.to_vec();
    cbc::Decryptor::<Aes128>::new(&PACK_KEY.into(), &PACK_IV.into())
        .decrypt_padded::<NoPadding>(&mut plain)
        .map_err(|_| invalid("voice pack body is not a whole number of AES blocks"))?;
    Ok(plain)
}

/// Decompresses a 13-byte LZMA1 header plus raw stream. The 0xFF padding after the stream is
/// never read, because the header gives the unpacked size.
fn decompress(stream: &[u8]) -> io::Result<Vec<u8>> {
    let mut image = Vec::with_capacity(IMAGE_SIZE);
    lzma_rs::lzma_decompress(&mut io::BufReader::new(stream), &mut image)
        .map_err(|e| invalid(format!("voice pack body does not decompress: {e}")))?;
    Ok(image)
}

fn read_image(image: &[u8]) -> io::Result<(u32, u32, Vec<Vec<u8>>)> {
    let version = u32_at(image, 0);
    let count = u32_at(image, 4) as usize;
    if count != SLOT_COUNT {
        return Err(invalid(format!(
            "unpacked image has {count} slots, expected {SLOT_COUNT}"
        )));
    }
    let table_end = 8 + 8 * count;
    let entry = |slot: usize| (u32_at(image, 8 + 8 * slot), u32_at(image, 12 + 8 * slot));
    let base = entry(0)
        .1
        .checked_sub(table_end as u32)
        .ok_or_else(|| invalid("slot 0 starts before the end of the slot table"))?;

    let mut clips = Vec::with_capacity(count);
    for slot in 0..count {
        let (size, offset) = entry(slot);
        let start = offset
            .checked_sub(base)
            .map(|start| start as usize)
            .filter(|&start| start >= table_end);
        let clip = start
            .and_then(|start| image.get(start..start + size as usize))
            .ok_or_else(|| invalid(format!("slot {slot} lies outside the unpacked image")))?;
        clips.push(clip.to_vec());
    }
    Ok((version, base, clips))
}

fn u16_at(bytes: &[u8], pos: usize) -> u16 {
    u16::from_le_bytes(bytes[pos..pos + 2].try_into().unwrap())
}

fn u32_at(bytes: &[u8], pos: usize) -> u32 {
    u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap())
}
