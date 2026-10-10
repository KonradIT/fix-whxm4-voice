//! Airoha's RACE protocol, as the headphones speak it on the `Airoha_APP` channel.
//!
//! See "RACE framing" in `docs/upload-protocol.md`. All values are little-endian.

use std::collections::VecDeque;
use std::io;

use crate::fota;
use crate::invalid;
use crate::link::{Link, Transport, hex};

/// A command that expects a reply.
pub const COMMAND: u8 = 0x5A;
/// A reply.
pub const RESPONSE: u8 = 0x5B;
/// A command without a reply.
pub const COMMAND_NO_REPLY: u8 = 0x5C;
/// An unrequested message, which also completes some FOTA stages.
pub const INDICATION: u8 = 0x5D;

/// One packet on the channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Packet {
    /// `0x05`, with `0x10` added for FOTA-engine packets.
    pub head: u8,
    /// [`COMMAND`], [`RESPONSE`], [`COMMAND_NO_REPLY`] or [`INDICATION`].
    pub kind: u8,
    pub id: u16,
    pub payload: Vec<u8>,
}

/// The bytes that carry `packet`.
pub fn encode(packet: &Packet) -> Vec<u8> {
    let mut bytes = vec![packet.head, packet.kind];
    bytes.extend((2 + packet.payload.len() as u16).to_le_bytes());
    bytes.extend(packet.id.to_le_bytes());
    bytes.extend(&packet.payload);
    bytes
}

/// Collects received bytes into packets, using each packet's length field. A packet may arrive
/// split across reads.
#[derive(Default)]
pub struct Decoder {
    buffer: Vec<u8>,
}

impl Decoder {
    /// Adds received bytes and returns the packets they complete.
    pub fn push(&mut self, bytes: &[u8]) -> io::Result<Vec<Packet>> {
        self.buffer.extend(bytes);
        let mut packets = Vec::new();
        while self.buffer.len() >= 4 {
            let length = u16::from_le_bytes([self.buffer[2], self.buffer[3]]) as usize;
            if length < 2 {
                return Err(invalid(format!(
                    "RACE packet too short: {}",
                    hex(&self.buffer)
                )));
            }
            if self.buffer.len() < 4 + length {
                break;
            }
            let raw: Vec<u8> = self.buffer.drain(..4 + length).collect();
            packets.push(Packet {
                head: raw[0],
                kind: raw[1],
                id: u16::from_le_bytes([raw[4], raw[5]]),
                payload: raw[6..].to_vec(),
            });
        }
        Ok(packets)
    }
}

/// A RACE channel: frames commands and picks out the reply to each, the packet with the command's ID
/// that completes its stage (see [`fota::reply_kind`]). A response with an error status completes it
/// early, and a cancel from the headphones fails it.
pub struct RaceLink<T> {
    transport: T,
    decoder: Decoder,
    received: VecDeque<Packet>,
}

impl<T: Transport> RaceLink<T> {
    pub fn new(transport: T) -> Self {
        RaceLink {
            transport,
            decoder: Decoder::default(),
            received: VecDeque::new(),
        }
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    fn next_packet(&mut self) -> io::Result<Packet> {
        loop {
            if let Some(packet) = self.received.pop_front() {
                return Ok(packet);
            }
            let bytes = self.transport.receive()?;
            self.received.extend(self.decoder.push(&bytes)?);
        }
    }
}

impl<T: Transport> Link<Packet> for RaceLink<T> {
    fn request(&mut self, command: &Packet) -> io::Result<Packet> {
        self.transport.send(&encode(command))?;
        loop {
            let packet = self.next_packet().map_err(|e| match e.kind() {
                io::ErrorKind::TimedOut => io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("no RACE reply to {:#06x}", command.id),
                ),
                _ => e,
            })?;
            if packet.id == fota::CANCEL && packet.kind == COMMAND {
                return Err(io::Error::other(format!(
                    "the headphones cancelled the update in reply to {:#06x}: {}",
                    command.id,
                    fota::cancel_reason(&packet.payload)
                )));
            }
            if packet.id != command.id {
                continue;
            }
            let refused = packet.kind == RESPONSE && packet.payload.first() != Some(&0);
            if packet.kind == fota::reply_kind(command.id) || refused {
                return Ok(packet);
            }
        }
    }
}
