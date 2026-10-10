//! Airoha's FOTA commands over RACE, as the Sony app sends them to install a voice pack.
//!
//! See "Commands" in `docs/upload-protocol.md`. Its byte offsets count from the start of the packet;
//! the offsets here count from the payload, 6 bytes later, so the status is `payload[0]`.

use std::io;
use std::time::Duration;

use crate::invalid;
use crate::link::{REPLY_TIMEOUT, hex};
use crate::race::{self, Packet};

/// The head byte of every packet the FOTA engine sends.
pub const HEAD: u8 = 0x15;

/// Read an NV key; the app reads the chip name with it.
pub const READ_NV_KEY: u16 = 0x0A00;
pub const START: u16 = 0x1C08;
pub const TRANSMIT_INTERVAL: u16 = 0x1C1C;
pub const PARTITION_INFO: u16 = 0x1C00;
pub const QUERY_STATE: u16 = 0x1C04;
pub const ACTIVE_FOTA: u16 = 0x1C19;
pub const ERASE_STATUS: u16 = 0x0433;
pub const COMPARE: u16 = 0x0431;
pub const START_TRANSACTION: u16 = 0x1C0A;
pub const WRITE_STATE: u16 = 0x1C06;
pub const ERASE: u16 = 0x0404;
pub const PAGE_PROGRAM: u16 = 0x0402;
pub const CHECK_INTEGRITY: u16 = 0x1C01;
pub const COMMIT: u16 = 0x1C02;
/// Sent by the headphones (as a command) when they refuse or abort the update.
pub const CANCEL: u16 = 0x1C03;

/// The commands whose stage is completed by an indication rather than a response.
const COMPLETED_BY_INDICATION: [u16; 7] = [
    START,
    TRANSMIT_INTERVAL,
    ACTIVE_FOTA,
    ERASE_STATUS,
    COMPARE,
    ERASE,
    CHECK_INTEGRITY,
];

/// The FOTA states the app writes with [`WRITE_STATE`], and reads with [`QUERY_STATE`].
pub const STATE_ERASING: u16 = 0x0200;
pub const STATE_ERASED: u16 = 0x0201;
pub const STATE_PROGRAMMING: u16 = 0x0210;
/// The integrity check passed; the image waits for a commit.
pub const STATE_CHECKED: u16 = 0x0211;
/// No state yet.
pub const STATE_NONE: u16 = 0xFFFF;

/// Erase works in blocks of this size, and the pack is split into them.
pub const BLOCK_SIZE: usize = 0x1000;
/// Program works in pages of this size.
pub const PAGE_SIZE: usize = 0x100;
/// The most bytes one erase-status request may cover.
pub const ERASE_STATUS_MAX: usize = 0x20_0000;

/// `len` rounded up to whole 4 KB blocks: the range an upload of `len` bytes may erase.
pub fn rounded(len: usize) -> usize {
    len.div_ceil(BLOCK_SIZE) * BLOCK_SIZE
}

/// How long the app waits after the commit for the headphones to restart, which drops the link.
pub const COMMIT_TIMEOUT: Duration = Duration::from_secs(15);

/// How long to wait for the reply to command `id`: the commit waits as long as the app does.
pub fn reply_timeout(id: u16) -> Duration {
    if id == COMMIT {
        COMMIT_TIMEOUT
    } else {
        REPLY_TIMEOUT
    }
}

/// The packet type that completes the stage of command `id`: an indication or a response.
pub fn reply_kind(id: u16) -> u8 {
    if COMPLETED_BY_INDICATION.contains(&id) {
        race::INDICATION
    } else {
        race::RESPONSE
    }
}

/// What a [`CANCEL`] from the headphones gives as its reason, byte 2 of its payload.
pub fn cancel_reason(payload: &[u8]) -> String {
    match payload.get(2) {
        Some(0) => "cancelled".into(),
        Some(1) => "failed".into(),
        Some(2) => "timed out".into(),
        Some(3) => "partner lost".into(),
        Some(4) => "not allowed".into(),
        _ => format!("reason unknown: {}", hex(payload)),
    }
}

/// The FOTA partition, as the headphones report it in reply to [`PARTITION_INFO`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Partition {
    pub id: u8,
    /// The storage-type byte every erase, compare and program command carries.
    pub storage: u8,
    pub address: u32,
    pub length: u32,
}

impl Partition {
    /// Reads the reply to [`PARTITION_INFO`]: status, ID, storage type, u32 address, u32 length.
    pub fn parse(reply: &Packet) -> io::Result<Self> {
        check_status(reply, "partition info")?;
        let p = field(reply, 0, 11, "partition info")?;
        Ok(Partition {
            id: p[1],
            storage: p[2],
            address: u32_at(p, 3),
            length: u32_at(p, 7),
        })
    }
}

/// Fails unless `reply` has status 0, naming the step as `what`.
pub fn check_status(reply: &Packet, what: &str) -> io::Result<()> {
    match reply.payload.first() {
        Some(0) => Ok(()),
        Some(status) => Err(io::Error::other(format!(
            "the headphones answered {what} ({:#06x}) with status {status:#04x}",
            reply.id
        ))),
        None => Err(invalid(format!(
            "the headphones answered {what} ({:#06x}) without a status",
            reply.id
        ))),
    }
}

/// The bytes `start..end` of `reply`'s payload, or an error naming the step.
pub fn field<'a>(reply: &'a Packet, start: usize, end: usize, what: &str) -> io::Result<&'a [u8]> {
    reply.payload.get(start..end).ok_or_else(|| {
        invalid(format!(
            "the reply to {what} ({:#06x}) is too short: {}",
            reply.id,
            hex(&reply.payload)
        ))
    })
}

/// The little-endian u32 at `at` in `bytes`, which must be long enough.
pub fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}

/// The state in a reply to [`QUERY_STATE`].
pub fn parse_state(reply: &Packet) -> io::Result<u16> {
    check_status(reply, "the state query")?;
    let p = field(reply, 1, 3, "the state query")?;
    Ok(u16::from_le_bytes([p[0], p[1]]))
}

fn command(head: u8, id: u16, payload: Vec<u8>) -> Packet {
    Packet {
        head,
        kind: race::COMMAND,
        id,
        payload,
    }
}

/// The NV-key read of the chip name (key `0x1002`, at most 1000 bytes). It is read-only, and the
/// headphones answer it outside language update mode too, with head `0x05` as ticket 08 sent it.
pub fn read_chip_name() -> Packet {
    command(0x05, READ_NV_KEY, vec![0x02, 0x10, 0xE8, 0x03])
}

/// Opens a FOTA session for a single device.
pub fn start() -> Packet {
    command(HEAD, START, vec![0x01])
}

pub fn transmit_interval() -> Packet {
    command(HEAD, TRANSMIT_INTERVAL, vec![0x01, 0x00])
}

pub fn partition_info() -> Packet {
    command(HEAD, PARTITION_INFO, vec![0x00])
}

pub fn query_state() -> Packet {
    command(HEAD, QUERY_STATE, Vec::new())
}

/// The single-device `TwsActiveFota` the app sends in foreground, non-TWS mode.
pub fn active_fota() -> Packet {
    command(HEAD, ACTIVE_FOTA, vec![0x00])
}

fn storage_range(storage: u8, address: u32, length: u32) -> Vec<u8> {
    let mut payload = vec![storage, 0x00];
    payload.extend(address.to_le_bytes());
    payload.extend(length.to_le_bytes());
    payload
}

/// Asks which 4 KB blocks of `length` bytes from `address` are erased.
pub fn erase_status(storage: u8, address: u32, length: u32) -> Packet {
    command(HEAD, ERASE_STATUS, storage_range(storage, address, length))
}

/// Asks for the SHA-256 of `length` bytes of flash from `address`.
pub fn compare(storage: u8, address: u32, length: u32) -> Packet {
    command(HEAD, COMPARE, storage_range(storage, address, length))
}

pub fn start_transaction() -> Packet {
    command(HEAD, START_TRANSACTION, Vec::new())
}

pub fn write_state(state: u16) -> Packet {
    command(HEAD, WRITE_STATE, state.to_le_bytes().to_vec())
}

/// Erases the 4 KB block at `address`.
pub fn erase(storage: u8, address: u32) -> Packet {
    let mut payload = vec![storage];
    payload.extend((BLOCK_SIZE as u32).to_le_bytes());
    payload.extend(address.to_le_bytes());
    command(HEAD, ERASE, payload)
}

/// Programs one 256-byte page at `address`.
pub fn page_program(storage: u8, address: u32, page: &[u8; PAGE_SIZE]) -> Packet {
    let mut payload = vec![storage, 1, page_crc(page)];
    payload.extend(address.to_le_bytes());
    payload.extend(page);
    command(HEAD, PAGE_PROGRAM, payload)
}

pub fn check_integrity(storage: u8) -> Packet {
    command(HEAD, CHECK_INTEGRITY, vec![0x01, 0x00, storage])
}

pub fn commit() -> Packet {
    command(HEAD, COMMIT, vec![0x00])
}

/// The checksum of a page: CRC-8 with polynomial `0x31`, seeded with 0, most significant bit
/// first, then bit-reversed.
pub fn page_crc(bytes: &[u8]) -> u8 {
    let crc = bytes.iter().fold(0u8, |crc, &b| {
        (0..8).fold(crc ^ b, |crc, _| {
            if crc & 0x80 != 0 {
                (crc << 1) ^ 0x31
            } else {
                crc << 1
            }
        })
    });
    crc.reverse_bits()
}
