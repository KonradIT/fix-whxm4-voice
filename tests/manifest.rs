use aes::Aes128;
use aes::cipher::{BlockCipherEncrypt, KeyInit};
use fix_whxm4_voice::keys::MANIFEST_KEY;
use fix_whxm4_voice::manifest::Manifest;
use sha1::{Digest, Sha1};

const MODEL: &str = "HP002";
const PACK_ID: &str = "VGIDLPB0401";
const PACK: &[u8] = b"pretend this is a voice pack";
const URI: &str =
    "https://info.update.sony.net/HP002/VGIDLPB0401/contents/0002/VP_english_UPG_03.bin";

fn sha1_hex(data: &[u8]) -> String {
    Sha1::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn xml(pack: &[u8]) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><InformationFile Version="1.0">
    <Distributions>
        <Distribution ID="Disclaimer" MAC="2455d4c5cf8cf3488fa8b2a2ed36133d2aacf4d0" Size="2019" URI="x"/>
        <Distribution ID="FW" InstallType="binary" MAC="{}" Size="{}" Type="" URI="{URI}" Version="3"/>
    </Distributions>
</InformationFile>"#,
        sha1_hex(pack),
        pack.len()
    )
}

/// A manifest the way Sony's server sends it: header lines, a blank line, then the XML zero-padded
/// and encrypted with AES-128-ECB.
fn encode(xml: &str, model: &str, pack_id: &str) -> Vec<u8> {
    let mut body = xml.as_bytes().to_vec();
    body.resize(body.len().div_ceil(16) * 16, 0);
    let digest = sha1_hex(format!("{}{pack_id}{model}", sha1_hex(&body)).as_bytes());
    let cipher = Aes128::new(&MANIFEST_KEY.into());
    for block in body.as_chunks_mut::<16>().0 {
        cipher.encrypt_block(block.into());
    }
    let mut out = format!("eaid:ENC0003\ndaid:HAS0003\ndigest:{digest}\n\n").into_bytes();
    out.extend(body);
    out
}

#[test]
fn decodes_the_pack_distribution() {
    let manifest = Manifest::decode(&encode(&xml(PACK), MODEL, PACK_ID)).unwrap();

    assert_eq!(manifest.pack.uri, URI);
    assert_eq!(manifest.pack.size, PACK.len() as u64);
    assert_eq!(manifest.pack.sha1[..], Sha1::digest(PACK)[..]);
}

#[test]
fn digest_matches_the_model_and_pack_id_it_was_made_for() {
    let manifest = Manifest::decode(&encode(&xml(PACK), MODEL, PACK_ID)).unwrap();

    manifest.check_digest(MODEL, PACK_ID).unwrap();
    assert!(manifest.check_digest(MODEL, "VGIDLPB0402").is_err());
}

#[test]
fn tampered_body_fails_the_digest() {
    let mut data = encode(&xml(PACK), MODEL, PACK_ID);
    let last = data.len() - 1;
    data[last] ^= 1;

    let digest_ok = Manifest::decode(&data).is_ok_and(|m| m.check_digest(MODEL, PACK_ID).is_ok());

    assert!(!digest_ok);
}

#[test]
fn accepts_the_pack_it_describes() {
    let manifest = Manifest::decode(&encode(&xml(PACK), MODEL, PACK_ID)).unwrap();

    manifest.check_pack(PACK).unwrap();
}

#[test]
fn rejects_a_pack_of_the_wrong_size() {
    let manifest = Manifest::decode(&encode(&xml(PACK), MODEL, PACK_ID)).unwrap();

    assert!(manifest.check_pack(&PACK[1..]).is_err());
}

#[test]
fn rejects_a_pack_with_the_wrong_sha1() {
    let manifest = Manifest::decode(&encode(&xml(PACK), MODEL, PACK_ID)).unwrap();
    let mut other = PACK.to_vec();
    other[0] ^= 1;

    assert!(manifest.check_pack(&other).is_err());
}

#[test]
fn rejects_a_manifest_without_a_pack_distribution() {
    let xml = xml(PACK).replace(r#"ID="FW""#, r#"ID="Other""#);

    assert!(Manifest::decode(&encode(&xml, MODEL, PACK_ID)).is_err());
}

#[test]
fn rejects_malformed_manifests() {
    let good = encode(&xml(PACK), MODEL, PACK_ID);

    assert!(Manifest::decode(b"").is_err());
    assert!(Manifest::decode(b"eaid:ENC0003\ndaid:HAS0003\n").is_err());
    assert!(Manifest::decode(&good[..good.len() - 1]).is_err());
}
