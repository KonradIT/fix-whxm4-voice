//! Sony's MDR control protocol, as the headphones speak it on the `Serial HPC` channel.
//!
//! See "Sony MDR commands around the transfer" in `docs/upload-protocol.md`.

use std::collections::VecDeque;
use std::io;

use crate::invalid;
use crate::link::{Link, REPLY_TIMEOUT, Transport, hex};

/// The data type of an acknowledgement.
pub const ACK: u8 = 0x01;
/// The data type of table-set-1 commands, such as the protocol info and the device info.
pub const DATA_MDR: u8 = 0x0C;
/// The data type of table-set-2 commands, such as voice guidance.
pub const DATA_MDR_NO2: u8 = 0x0E;

const START: u8 = 0x3E;
const END: u8 = 0x3C;
const ESCAPE: u8 = 0x3D;

/// A command or reply, without the framing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub data_type: u8,
    pub payload: Vec<u8>,
}

/// One frame on the channel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub data_type: u8,
    /// 0 or 1. The receiver acknowledges a frame with the other number.
    pub seq: u8,
    pub payload: Vec<u8>,
}

/// The bytes that carry `frame`: start marker, escaped content, end marker.
pub fn encode(frame: &Frame) -> Vec<u8> {
    let mut content = vec![frame.data_type, frame.seq];
    content.extend((frame.payload.len() as u32).to_be_bytes());
    content.extend(&frame.payload);
    content.push(checksum(&content));

    let mut bytes = vec![START];
    for b in content {
        if matches!(b, START | END | ESCAPE) {
            bytes.extend([ESCAPE, b & !0x10]);
        } else {
            bytes.push(b);
        }
    }
    bytes.push(END);
    bytes
}

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |sum, &b| sum.wrapping_add(b))
}

/// Collects received bytes into frames. A frame may arrive split across reads.
#[derive(Default)]
pub struct Decoder {
    buffer: Vec<u8>,
}

impl Decoder {
    /// Adds received bytes and returns the frames they complete.
    pub fn push(&mut self, bytes: &[u8]) -> io::Result<Vec<Frame>> {
        self.buffer.extend(bytes);
        let mut frames = Vec::new();
        while let Some(end) = self.buffer.iter().position(|&b| b == END) {
            let raw: Vec<u8> = self.buffer.drain(..=end).collect();
            // Anything before the start marker is noise from a frame that began before we listened.
            let Some(start) = raw.iter().rposition(|&b| b == START) else {
                continue;
            };
            frames.push(decode(&raw[start + 1..raw.len() - 1])?);
        }
        Ok(frames)
    }
}

fn decode(escaped: &[u8]) -> io::Result<Frame> {
    let mut content = Vec::with_capacity(escaped.len());
    let mut bytes = escaped.iter();
    while let Some(&b) = bytes.next() {
        if b == ESCAPE {
            let &next = bytes
                .next()
                .ok_or_else(|| invalid("MDR frame ends in an escape"))?;
            content.push(next | 0x10);
        } else {
            content.push(b);
        }
    }

    let Some((&sum, body)) = content.split_last() else {
        return Err(invalid("empty MDR frame"));
    };
    if body.len() < 6 {
        return Err(invalid(format!("MDR frame too short: {}", hex(&content))));
    }
    if checksum(body) != sum {
        return Err(invalid(format!(
            "MDR frame with a wrong checksum: {}",
            hex(&content)
        )));
    }
    let length = u32::from_be_bytes(body[2..6].try_into().unwrap()) as usize;
    if body.len() != 6 + length {
        return Err(invalid(format!(
            "MDR frame with a wrong length: {}",
            hex(&content)
        )));
    }
    Ok(Frame {
        data_type: body[0],
        seq: body[1],
        payload: body[6..].to_vec(),
    })
}

/// Whether `reply` answers `command`: a GET command's reply starts with the command byte plus one,
/// then repeats the rest of the command, such as `47 01 02 …` for `46 01 02`.
fn answers(command: &Message, reply: &Message) -> bool {
    let (Some((&id, rest)), Some((&reply_id, reply_rest))) =
        (command.payload.split_first(), reply.payload.split_first())
    else {
        return false;
    };
    reply.data_type == command.data_type
        && reply_id == id.wrapping_add(1)
        && reply_rest.starts_with(rest)
}

/// An MDR channel: numbers and frames commands, acknowledges everything the headphones send, and
/// picks out the reply to each command.
pub struct MdrLink<T> {
    transport: T,
    seq: u8,
    decoder: Decoder,
    received: VecDeque<Frame>,
}

impl<T: Transport> MdrLink<T> {
    pub fn new(transport: T) -> Self {
        MdrLink {
            transport,
            seq: 0,
            decoder: Decoder::default(),
            received: VecDeque::new(),
        }
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    fn next_frame(&mut self) -> io::Result<Frame> {
        loop {
            if let Some(frame) = self.received.pop_front() {
                return Ok(frame);
            }
            let bytes = self.transport.receive(REPLY_TIMEOUT)?;
            self.received.extend(self.decoder.push(&bytes)?);
        }
    }
}

impl<T: Transport> Link<Message> for MdrLink<T> {
    fn request(&mut self, command: &Message) -> io::Result<Message> {
        self.transport.send(&encode(&Frame {
            data_type: command.data_type,
            seq: self.seq,
            payload: command.payload.clone(),
        }))?;
        // The next command must carry the seq of this one's ACK, so wait for both, in either order.
        let mut acked = false;
        let mut reply = None;
        while !acked || reply.is_none() {
            let frame = self.next_frame().map_err(|e| match e.kind() {
                io::ErrorKind::TimedOut => io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("no MDR reply to {}", hex(&command.payload)),
                ),
                _ => e,
            })?;
            if frame.data_type == ACK {
                self.seq = frame.seq;
                acked = true;
                continue;
            }
            self.transport.send(&encode(&Frame {
                data_type: ACK,
                seq: 1 - (frame.seq & 1),
                payload: Vec::new(),
            }))?;
            let message = Message {
                data_type: frame.data_type,
                payload: frame.payload,
            };
            if reply.is_none() && answers(command, &message) {
                reply = Some(message);
            }
        }
        Ok(reply.expect("the loop ends with a reply"))
    }
}
