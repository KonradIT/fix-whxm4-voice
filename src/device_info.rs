//! What the headphones report about themselves, read with read-only queries only.

use std::fmt;
use std::io;

use crate::invalid;
use crate::link::{Link, hex};
use crate::mdr::{self, Message};
use crate::race::{self, Packet};

/// The read-only MDR queries listed in `docs/upload-protocol.md`. `device-info` can only send these.
#[derive(Clone, Copy, Debug)]
enum Query {
    ProtocolInfo,
    ModelName,
    FirmwareVersion,
    VoiceGuidanceCapability,
    UpdateModeStatus,
    CurrentLanguage,
    Languages,
}

impl Query {
    fn message(self) -> Message {
        let (data_type, payload): (u8, &[u8]) = match self {
            Query::ProtocolInfo => (mdr::DATA_MDR, &[0x00, 0x00]),
            Query::ModelName => (mdr::DATA_MDR, &[0x04, 0x01]),
            Query::FirmwareVersion => (mdr::DATA_MDR, &[0x04, 0x02]),
            Query::VoiceGuidanceCapability => (mdr::DATA_MDR_NO2, &[0x40, 0x01]),
            Query::UpdateModeStatus => (mdr::DATA_MDR_NO2, &[0x42, 0x01, 0x02]),
            Query::CurrentLanguage => (mdr::DATA_MDR_NO2, &[0x46, 0x01, 0x02]),
            Query::Languages => (mdr::DATA_MDR_NO2, &[0x46, 0x01, 0x04]),
        };
        Message {
            data_type,
            payload: payload.to_vec(),
        }
    }
}

/// The RACE NV-key read of the chip name (`0x0A00`, key `0x1002`, at most 1000 bytes), the one
/// read-only RACE command the headphones answer outside language update mode.
fn chip_name_query() -> Packet {
    Packet {
        head: 0x05,
        kind: race::COMMAND,
        id: 0x0A00,
        payload: vec![0x02, 0x10, 0xE8, 0x03],
    }
}

/// What the headphones report.
#[derive(Debug)]
pub struct DeviceInfo {
    /// `None` when the headphones don't answer the query.
    pub model: Option<String>,
    /// `None` when the headphones don't answer the query.
    pub firmware: Option<String>,
    /// The bytes after `01 00` in the protocol-info reply.
    pub protocol_info: Vec<u8>,
    /// The chip name from the RACE NV key, empty on the user's unit. `None` when the headphones
    /// refuse the read.
    pub chip_name: Option<String>,
    /// The active voice-guidance language, as an MDR language code.
    pub language: u8,
    /// Whether language update mode is on.
    pub update_mode: bool,
    /// The language codes the voice-guidance capability lists.
    pub supported: Vec<u8>,
    /// The update server category, such as `HP002`.
    pub category: String,
    pub serial: String,
    /// Each language the headphones offer, with its voice pack's service ID.
    pub packs: Vec<(u8, String)>,
}

/// Asks the headphones about themselves, sending only read-only queries.
pub fn read(mdr: &mut impl Link<Message>, race: &mut impl Link<Packet>) -> io::Result<DeviceInfo> {
    let mut ask = |query: Query| -> io::Result<Reply> {
        let message = query.message();
        let reply = mdr.request(&message)?;
        // Skip the reply's echo of the command, such as `47 01 02` for `46 01 02`.
        Ok(Reply {
            query,
            bytes: reply.payload,
            at: message.payload.len(),
        })
    };

    let protocol_info = ask(Query::ProtocolInfo)?.rest();
    let model = optional(ask(Query::ModelName))?
        .map(|mut r| r.string())
        .transpose()?;
    let firmware = optional(ask(Query::FirmwareVersion))?
        .map(|mut r| r.string())
        .transpose()?;

    let mut capability = ask(Query::VoiceGuidanceCapability)?;
    // Two bytes of unknown meaning (`01 01` on the user's unit), then a counted list of codes.
    capability.take(2)?;
    let count = capability.byte()? as usize;
    let supported = capability.take(count)?.to_vec();

    let update_mode = ask(Query::UpdateModeStatus)?.byte()? == 0x00;
    let language = ask(Query::CurrentLanguage)?.byte()?;

    let mut languages = ask(Query::Languages)?;
    languages.take(1)?;
    let category = languages.string()?;
    let serial = languages.string()?;
    let count = languages.byte()?;
    let packs = (0..count)
        .map(|_| Ok((languages.byte()?, languages.string()?)))
        .collect::<io::Result<_>>()?;

    let chip = race.request(&chip_name_query())?;
    // Status, u16 length, then the name. The status is 0 on success, as in every RACE reply.
    let chip_name = match chip.payload.as_slice() {
        [0, length_lo, length_hi, name @ ..] => {
            let length = u16::from_le_bytes([*length_lo, *length_hi]) as usize;
            Some(String::from_utf8_lossy(&name[..length.min(name.len())]).into_owned())
        }
        _ => None,
    };

    Ok(DeviceInfo {
        model,
        firmware,
        protocol_info,
        chip_name,
        language,
        update_mode,
        supported,
        category,
        serial,
        packs,
    })
}

/// `None` for a query the headphones didn't answer.
fn optional(reply: io::Result<Reply>) -> io::Result<Option<Reply>> {
    match reply {
        Ok(reply) => Ok(Some(reply)),
        Err(e) if e.kind() == io::ErrorKind::TimedOut => Ok(None),
        Err(e) => Err(e),
    }
}

/// A reply being read from front to back.
struct Reply {
    query: Query,
    bytes: Vec<u8>,
    at: usize,
}

impl Reply {
    fn take(&mut self, n: usize) -> io::Result<&[u8]> {
        if self.at + n > self.bytes.len() {
            return Err(invalid(format!(
                "reply to {:?} is too short: {}",
                self.query,
                hex(&self.bytes)
            )));
        }
        self.at += n;
        Ok(&self.bytes[self.at - n..self.at])
    }

    fn byte(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }

    /// A string preceded by its length byte.
    fn string(&mut self) -> io::Result<String> {
        let length = self.byte()? as usize;
        Ok(String::from_utf8_lossy(self.take(length)?).into_owned())
    }

    fn rest(self) -> Vec<u8> {
        self.bytes[self.at.min(self.bytes.len())..].to_vec()
    }
}

/// The name of an MDR language code, from `docs/upload-protocol.md`.
fn language_name(code: u8) -> &'static str {
    match code {
        0x00 => "undefined",
        0x01 => "English",
        0x02 => "French",
        0x03 => "German",
        0x04 => "Spanish",
        0x05 => "Italian",
        0x06 => "Portuguese",
        0x07 => "Dutch",
        0x08 => "Swedish",
        0x09 => "Finnish",
        0x0A => "Russian",
        0x0B => "Japanese",
        0x0C => "Simplified Chinese",
        0x0D => "Brazilian Portuguese",
        0x0E => "Traditional Chinese",
        0x0F => "Korean",
        0x10 => "Turkish",
        0xF0 => "Chinese",
        _ => "unknown",
    }
}

fn language(code: u8) -> String {
    format!("{} ({code:#04x})", language_name(code))
}

impl fmt::Display for DeviceInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let reported = |value: &Option<String>| value.clone().unwrap_or("not reported".into());
        writeln!(f, "Model:                {}", reported(&self.model))?;
        writeln!(f, "Firmware:             {}", reported(&self.firmware))?;
        writeln!(f, "MDR protocol info:    {}", hex(&self.protocol_info))?;
        let chip = match self.chip_name.as_deref() {
            None => "(the headphones refused the read)",
            Some("") => "(empty, the Sony app then assumes MT2811)",
            Some(name) => name,
        };
        writeln!(f, "Chip name:            {chip}")?;
        writeln!(f, "Current language:     {}", language(self.language))?;
        let mode = if self.update_mode { "on" } else { "off" };
        writeln!(f, "Language update mode: {mode}")?;
        let supported: Vec<String> = self.supported.iter().map(|&c| language(c)).collect();
        writeln!(f, "Supported languages:  {}", supported.join(", "))?;
        writeln!(f, "Update category:      {}", self.category)?;
        writeln!(f, "Serial number:        {}", self.serial)?;
        write!(f, "Voice packs:")?;
        for (code, service_id) in &self.packs {
            write!(f, "\n  {service_id}  {}", language(*code))?;
        }
        Ok(())
    }
}
