//! The upload: the Sony app's voice-pack update sequence, replayed over the device link.
//!
//! See "The voice-pack update sequence" in `docs/upload-protocol.md`. Every RACE command goes
//! through the [`Guard`], which refuses anything outside the sequence or the pack's range.

use std::fmt;
use std::io;

use sha2::{Digest, Sha256};

use crate::build::BuiltPack;
use crate::fota::{self, BLOCK_SIZE, ERASE_STATUS_MAX, PAGE_SIZE, Partition};
use crate::guard::{Guard, Writes};
use crate::invalid;
use crate::link::{Link, hex};
use crate::mdr::{self, Message};
use crate::race::Packet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Switch language update mode on, run the queries up to the transfer, and report what the
    /// transfer would do. Nothing is erased, written, checked or committed.
    DryRun,
    /// The whole sequence, up to and including the commit.
    Upload,
}

/// What happens to one 4 KB block of the pack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Flash already holds this block of the pack.
    Keep,
    /// Flash is erased here, so the block is only programmed.
    Program,
    EraseAndProgram,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub address: u32,
    /// The pack's bytes in this block: 4 KB, or less for the last one.
    pub length: usize,
    pub action: Action,
    /// The pages to program, those not entirely 0xFF; none for [`Action::Keep`].
    pub pages: usize,
}

/// What the headphones reported, and what the transfer does block by block.
#[derive(Clone, Debug)]
pub struct Plan {
    pub partition: Partition,
    /// The FOTA state before the transfer.
    pub state: u16,
    pub pack_len: usize,
    pub blocks: Vec<Block>,
}

impl Plan {
    /// The pages to program, over all blocks.
    pub fn pages(&self) -> usize {
        self.blocks.iter().map(|b| b.pages).sum()
    }

    /// The end of the range the upload may touch: the pack rounded up to 4 KB.
    fn end(&self) -> u64 {
        self.partition.address as u64 + fota::rounded(self.pack_len) as u64
    }
}

/// Runs the update sequence for `pack` in `mode`, switching language update mode on `mdr` and
/// sending the FOTA commands on `race` through the [`Guard`]. Returns the plan once the headphones'
/// checksum check passed and the commit was sent, or for a dry run, once update mode is off again.
pub fn run(
    mdr: &mut impl Link<Message>,
    race: &mut impl Link<Packet>,
    pack: &BuiltPack,
    mode: Mode,
) -> io::Result<Plan> {
    let writes = match mode {
        Mode::DryRun => Writes::Refused,
        Mode::Upload => Writes::Allowed,
    };
    let mut race = Guard::new(race, pack.bytes().len(), writes);

    let transferred = set_update_mode(mdr, true).and_then(|()| transfer(&mut race, pack, mode));
    // The app switches update mode off between the transfer and the install; after a failure,
    // switch it off all the same.
    let off = set_update_mode(mdr, false);
    let plan = transferred?;
    off?;

    if mode == Mode::Upload {
        let committed = set_update_mode(mdr, true).and_then(|()| commit(&mut race));
        if committed.is_err() {
            let _ = set_update_mode(mdr, false);
        }
        committed?;
    }
    Ok(plan)
}

/// Switches the headphones' language update mode, without which they refuse the FOTA queries.
fn set_update_mode(mdr: &mut impl Link<Message>, on: bool) -> io::Result<()> {
    let (value, name) = if on { (0x00, "on") } else { (0x01, "off") };
    let set = [0x44, 0x01, 0x02, value];
    let reply = mdr
        .request(&Message {
            data_type: mdr::DATA_MDR_NO2,
            payload: set.to_vec(),
        })
        .map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("cannot switch language update mode {name}: {e}"),
            )
        })?;
    if reply.payload != [0x45, 0x01, 0x02, value] {
        return Err(invalid(format!(
            "the headphones answered switching language update mode {name} with {}",
            hex(&reply.payload)
        )));
    }
    Ok(())
}

/// Steps 2 to 4 of the sequence: the queries, then for an upload the transfer up to the
/// integrity check and the final state query.
fn transfer<L: Link<Packet>>(
    race: &mut Guard<L>,
    pack: &BuiltPack,
    mode: Mode,
) -> io::Result<Plan> {
    // The chip name only picks the app's chip type, and the headphones report none.
    race.request(&fota::read_chip_name())?;
    fota::check_status(&race.request(&fota::start())?, "FOTA start")?;
    match race.request(&fota::transmit_interval()) {
        // The app skips this stage when it times out.
        Err(e) if e.kind() == io::ErrorKind::TimedOut => {}
        reply => fota::check_status(&reply?, "the transmit-interval query")?,
    }
    let partition = Partition::parse(&race.request(&fota::partition_info())?)?;
    let state = fota::parse_state(&race.request(&fota::query_state())?)?;
    if state == fota::STATE_CHECKED {
        // The app would skip the transfer and commit whatever is staged.
        return Err(io::Error::other(format!(
            "the FOTA partition at {:#x} holds an image that passed the headphones' check but was never \
             committed; the Sony app would commit that image instead of this pack, so nothing is sent",
            partition.address
        )));
    }
    let bytes = pack.bytes();
    if fota::rounded(bytes.len()) > partition.length as usize {
        return Err(io::Error::other(format!(
            "the pack takes {:#x} bytes rounded up to 4 KB, more than the FOTA partition's {:#x}",
            fota::rounded(bytes.len()),
            partition.length
        )));
    }
    fota::check_status(
        &race.request(&fota::active_fota())?,
        "the active-FOTA query",
    )?;

    let erased = erase_status(race, partition, bytes.len())?;
    let mut blocks = Vec::new();
    for (k, data) in bytes.chunks(BLOCK_SIZE).enumerate() {
        let address = partition.address + (k * BLOCK_SIZE) as u32;
        let action = if erased[k] {
            Action::Program
        } else {
            let reply = race.request(&fota::compare(
                partition.storage,
                address,
                data.len() as u32,
            ))?;
            fota::check_status(&reply, "compare")?;
            if fota::field(&reply, 11, 43, "compare")? == Sha256::digest(data).as_slice() {
                Action::Keep
            } else {
                Action::EraseAndProgram
            }
        };
        let pages = match action {
            Action::Keep => 0,
            _ => pages(data, address).count(),
        };
        blocks.push(Block {
            address,
            length: data.len(),
            action,
            pages,
        });
    }
    let plan = Plan {
        partition,
        state,
        pack_len: bytes.len(),
        blocks,
    };
    if mode == Mode::Upload {
        write(race, &plan, bytes)?;
    }
    Ok(plan)
}

/// Asks which 4 KB blocks of the pack's range are erased, at most [`ERASE_STATUS_MAX`] at a time.
fn erase_status<L: Link<Packet>>(
    race: &mut Guard<L>,
    partition: Partition,
    pack_len: usize,
) -> io::Result<Vec<bool>> {
    let size = fota::rounded(pack_len);
    let mut erased = Vec::new();
    for start in (0..size).step_by(ERASE_STATUS_MAX) {
        let length = ERASE_STATUS_MAX.min(size - start);
        let address = partition.address + start as u32;
        let reply = race.request(&fota::erase_status(
            partition.storage,
            address,
            length as u32,
        ))?;
        fota::check_status(&reply, "the erase-status query")?;
        let size = fota::field(&reply, 11, 13, "the erase-status query")?;
        let size = u16::from_le_bytes([size[0], size[1]]) as usize;
        let bitmap = fota::field(&reply, 13, 13 + size, "the erase-status query")?;
        let blocks = length / BLOCK_SIZE;
        if bitmap.len() * 8 < blocks {
            return Err(invalid(format!(
                "the erase status of {blocks} blocks at {address:#x} has only {} bits",
                bitmap.len() * 8
            )));
        }
        erased.extend((0..blocks).map(|k| bitmap[k / 8] & (0x80 >> (k % 8)) != 0));
    }
    Ok(erased)
}

/// The pages of `data` at `address`, a short last one padded with 0xFF, without those that are
/// entirely 0xFF: erased flash already holds them.
fn pages(data: &[u8], address: u32) -> impl Iterator<Item = (u32, [u8; PAGE_SIZE])> + '_ {
    data.chunks(PAGE_SIZE)
        .enumerate()
        .filter_map(move |(i, chunk)| {
            let mut page = [0xFF; PAGE_SIZE];
            page[..chunk.len()].copy_from_slice(chunk);
            let address = address + (i * PAGE_SIZE) as u32;
            (page != [0xFF; PAGE_SIZE]).then_some((address, page))
        })
}

/// The transfer proper: erase, program, integrity check, and the state the app leaves behind.
fn write<L: Link<Packet>>(race: &mut Guard<L>, plan: &Plan, pack: &[u8]) -> io::Result<()> {
    let storage = plan.partition.storage;
    fota::check_status(
        &race.request(&fota::start_transaction())?,
        "start transaction",
    )?;

    let erase: Vec<&Block> = plan
        .blocks
        .iter()
        .filter(|b| b.action == Action::EraseAndProgram)
        .collect();
    if !erase.is_empty() {
        write_state(race, fota::STATE_ERASING)?;
        for block in erase {
            let reply = race.request(&fota::erase(storage, block.address))?;
            fota::check_status(&reply, "erase")?;
            confirm_address(&reply, 6, block.address, "erase")?;
        }
    }

    let program: Vec<&Block> = plan
        .blocks
        .iter()
        .filter(|b| b.action != Action::Keep)
        .collect();
    if !program.is_empty() {
        write_state(race, fota::STATE_ERASED)?;
        write_state(race, fota::STATE_PROGRAMMING)?;
        for block in program {
            let offset = (block.address - plan.partition.address) as usize;
            for (address, page) in pages(&pack[offset..offset + block.length], block.address) {
                let reply = race.request(&fota::page_program(storage, address, &page))?;
                fota::check_status(&reply, "page program")?;
                confirm_address(&reply, 3, address, "page program")?;
            }
        }
    }

    let check = race.request(&fota::check_integrity(storage))?;
    if let Err(e) = fota::check_status(&check, "the checksum check") {
        return Err(io::Error::other(format!(
            "the headphones' checksum check failed, so they did not accept the pack and nothing was \
             committed: {e}"
        )));
    }
    write_state(race, fota::STATE_CHECKED)?;
    let state = fota::parse_state(&race.request(&fota::query_state())?)?;
    if state != fota::STATE_CHECKED {
        return Err(io::Error::other(format!(
            "after the checksum check, the headphones report FOTA state {state:#06x}, not {:#06x}",
            fota::STATE_CHECKED
        )));
    }
    Ok(())
}

fn write_state<L: Link<Packet>>(race: &mut Guard<L>, state: u16) -> io::Result<()> {
    let reply = race.request(&fota::write_state(state))?;
    fota::check_status(&reply, &format!("writing state {state:#06x}"))
}

/// Fails unless the u32 at `at` in `reply`'s payload is `address`.
fn confirm_address(reply: &Packet, at: usize, address: u32, what: &str) -> io::Result<()> {
    let confirmed = fota::u32_at(fota::field(reply, at, at + 4, what)?, 0);
    if confirmed != address {
        return Err(invalid(format!(
            "the headphones confirmed {what} at {confirmed:#x}, not at {address:#x}"
        )));
    }
    Ok(())
}

/// Commits the staged pack. The headphones restart to install it, which drops the link: the app
/// takes that as success.
fn commit<L: Link<Packet>>(race: &mut Guard<L>) -> io::Result<()> {
    match race.request(&fota::commit()) {
        Ok(reply) => fota::check_status(&reply, "the commit"),
        Err(e)
            if matches!(
                e.kind(),
                io::ErrorKind::UnexpectedEof
                    | io::ErrorKind::ConnectionReset
                    | io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::BrokenPipe
                    | io::ErrorKind::NotConnected
            ) =>
        {
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// The blocks with `action`, as merged address ranges.
fn ranges(blocks: &[Block], action: Action) -> String {
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for block in blocks.iter().filter(|b| b.action == action) {
        let end = block.address + BLOCK_SIZE as u32;
        match ranges.last_mut() {
            Some(last) if last.1 == block.address => last.1 = end,
            _ => ranges.push((block.address, end)),
        }
    }
    let ranges: Vec<String> = ranges
        .iter()
        .map(|(a, b)| format!("{a:#010x}..{b:#010x}"))
        .collect();
    ranges.join(", ")
}

impl fmt::Display for Plan {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let p = &self.partition;
        writeln!(
            f,
            "FOTA partition:  {:#010x}..{:#010x} ({:#x} bytes), ID {:#04x}, storage type {:#04x}",
            p.address,
            p.address as u64 + p.length as u64,
            p.length,
            p.id,
            p.storage
        )?;
        let state = match self.state {
            fota::STATE_NONE => " (none)",
            fota::STATE_ERASING | fota::STATE_ERASED | fota::STATE_PROGRAMMING => {
                " (an earlier transfer was interrupted)"
            }
            _ => " (not in the protocol notes)",
        };
        writeln!(f, "FOTA state:      {:#06x}{state}", self.state)?;
        writeln!(
            f,
            "Pack:            {} bytes, {} blocks of 4 KB",
            self.pack_len,
            self.blocks.len()
        )?;
        writeln!(
            f,
            "Planned range:   {:#010x}..{:#010x}, the pack rounded up to 4 KB",
            p.address,
            self.end()
        )?;
        let count = |action| self.blocks.iter().filter(|b| b.action == action).count();
        let line = |f: &mut fmt::Formatter, label: &str, action: Action, what: &str| {
            let n = count(action);
            if n == 0 {
                writeln!(f, "{label}none")
            } else {
                writeln!(
                    f,
                    "{label}{n} blocks {what}: {}",
                    ranges(&self.blocks, action)
                )
            }
        };
        line(
            f,
            "Unchanged:       ",
            Action::Keep,
            "already holding the pack",
        )?;
        line(f, "Already erased:  ", Action::Program, "to program only")?;
        line(
            f,
            "Erase:           ",
            Action::EraseAndProgram,
            "to erase and program",
        )?;
        write!(f, "Program:         {} pages of 256 bytes", self.pages())
    }
}
