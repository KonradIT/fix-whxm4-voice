//! The manifest codec: reads Sony's description of an available voice pack.
//!
//! A manifest is three header lines (`eaid`, `daid`, `digest`), a blank line, then an XML body
//! zero-padded and encrypted with AES-128-ECB. The voice pack is the `Distribution` with `ID="FW"`;
//! its `MAC` is the SHA-1 of the pack and `Size` its length in bytes.

use std::io;

use aes::Aes128;
use aes::cipher::{BlockCipherDecrypt, KeyInit};
use sha1::{Digest, Sha1};

use crate::invalid;
use crate::keys::MANIFEST_KEY;

pub struct Manifest {
    /// The `digest` header line.
    pub digest: String,
    /// The decrypted XML body, without its zero padding.
    pub xml: String,
    /// The voice pack this manifest describes.
    pub pack: Distribution,
    /// The decrypted body including its zero padding, which is what the digest covers.
    body: Vec<u8>,
}

pub struct Distribution {
    pub uri: String,
    pub sha1: [u8; 20],
    pub size: u64,
}

impl Manifest {
    pub fn decode(data: &[u8]) -> io::Result<Manifest> {
        let split = data
            .windows(2)
            .position(|w| w == b"\n\n")
            .ok_or_else(|| invalid("manifest has no blank line after its header"))?;
        let header = std::str::from_utf8(&data[..split])
            .map_err(|_| invalid("manifest header is not text"))?;
        let mut digest = None;
        for line in header.lines() {
            if let Some(value) = line.strip_prefix("digest:") {
                digest = Some(value.trim().to_lowercase());
            }
        }
        let digest = digest.ok_or_else(|| invalid("manifest header has no digest line"))?;

        let mut body = data[split + 2..].to_vec();
        if body.is_empty() || !body.len().is_multiple_of(16) {
            return Err(invalid("manifest body is not a whole number of AES blocks"));
        }
        let cipher = Aes128::new(&MANIFEST_KEY.into());
        for block in body.as_chunks_mut::<16>().0 {
            cipher.decrypt_block(block.into());
        }
        let unpadded = body.iter().rposition(|&b| b != 0).map_or(0, |i| i + 1);
        let xml = String::from_utf8(body[..unpadded].to_vec())
            .map_err(|_| invalid("manifest body does not decrypt to text"))?;
        let pack = pack_distribution(&xml)?;

        Ok(Manifest {
            digest,
            xml,
            pack,
            body,
        })
    }

    /// Checks the `digest` header, which covers the body and the server path it was served from:
    /// SHA-1 hex of (SHA-1 hex of the padded body + `pack_id` + `model`).
    pub fn check_digest(&self, model: &str, pack_id: &str) -> io::Result<()> {
        let expected = hex(&Sha1::digest(format!(
            "{}{pack_id}{model}",
            hex(&Sha1::digest(&self.body))
        )));
        if expected != self.digest {
            return Err(invalid(format!(
                "manifest digest {} does not match its body ({expected})",
                self.digest
            )));
        }
        Ok(())
    }

    /// Checks that `pack` is the voice pack this manifest describes, by size and SHA-1.
    pub fn check_pack(&self, pack: &[u8]) -> io::Result<()> {
        if pack.len() as u64 != self.pack.size {
            return Err(invalid(format!(
                "voice pack is {} bytes, the manifest says {}",
                pack.len(),
                self.pack.size
            )));
        }
        let sha1 = Sha1::digest(pack);
        if sha1[..] != self.pack.sha1 {
            return Err(invalid(format!(
                "voice pack SHA-1 is {}, the manifest says {}",
                hex(&sha1),
                hex(&self.pack.sha1)
            )));
        }
        Ok(())
    }
}

/// The `<Distribution ID="FW" …/>` element of the manifest XML.
fn pack_distribution(xml: &str) -> io::Result<Distribution> {
    let element = xml
        .split("<Distribution ")
        .skip(1)
        .map(|rest| rest.split('>').next().unwrap_or(rest))
        .find(|element| attribute(element, "ID") == Some("FW"))
        .ok_or_else(|| invalid("manifest has no voice pack distribution"))?;
    let get = |name| {
        attribute(element, name)
            .ok_or_else(|| invalid(format!("voice pack distribution has no {name}")))
    };

    let uri = get("URI")?.to_string();
    let size = get("Size")?
        .parse()
        .map_err(|_| invalid("voice pack distribution has an invalid Size"))?;
    let mac = get("MAC")?;
    let sha1 = parse_hex(mac)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| invalid(format!("voice pack distribution has an invalid MAC {mac}")))?;
    Ok(Distribution { uri, sha1, size })
}

/// The value of attribute `name` in the inside of an XML tag. Sony's attributes use double quotes.
fn attribute<'a>(element: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!(" {name}=\"");
    let start = format!(" {element}").find(&needle)? + needle.len() - 1;
    let value = &element[start..];
    value.split('"').next()
}

fn parse_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
