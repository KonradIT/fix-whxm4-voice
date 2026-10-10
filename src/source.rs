//! The pack source: fetches stock manifests and voice packs from Sony's public update server.

use std::io;

use crate::language::Language;
use crate::manifest::Manifest;

/// Sony's model path for the WH-1000XM4 on the update server.
const MODEL: &str = "HP002";
const SERVER: &str = "https://info.update.sony.net";

/// A stock manifest and the voice pack it describes, checked against each other.
pub struct Stock {
    pub manifest: Vec<u8>,
    pub pack: Vec<u8>,
}

/// Downloads the manifest and voice pack for `lang`, checking the manifest digest and the pack's
/// size and SHA-1.
pub fn fetch(lang: Language) -> io::Result<Stock> {
    let pack_id = lang.pack_id();
    let manifest = get(&format!("{SERVER}/{MODEL}/{pack_id}/info/info.xml"))?;
    let decoded = Manifest::decode(&manifest)?;
    decoded.check_digest(MODEL, &pack_id)?;
    if !decoded.pack.uri.starts_with(&format!("{SERVER}/")) {
        return Err(io::Error::other(format!(
            "manifest points the voice pack outside Sony's update server: {}",
            decoded.pack.uri
        )));
    }
    let pack = get(&decoded.pack.uri)?;
    decoded.check_pack(&pack)?;
    Ok(Stock { manifest, pack })
}

fn get(url: &str) -> io::Result<Vec<u8>> {
    eprintln!("downloading {url}");
    ureq::get(url)
        .call()
        .and_then(|mut response| response.body_mut().read_to_vec())
        .map_err(|e| io::Error::other(format!("{url}: {e}")))
}
