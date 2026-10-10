//! The range guard: the only way the upload reaches the RACE channel.
//!
//! It decodes every command itself, so a bug in the procedure above cannot get past it. Only the
//! commands of the voice-pack update sequence go through. Erase and program must lie inside the
//! FOTA partition the headphones reported, and inside the pack rounded up to 4 KB from its start;
//! the erase-status and compare queries obey the same bounds. Until the headphones have reported the
//! partition, nothing with an address goes through. See ADR 0001.

use std::io;
use std::ops::Range;

use crate::fota::{self, BLOCK_SIZE, PAGE_SIZE, Partition, u32_at};
use crate::link::{Link, hex};
use crate::race::{self, Packet};

/// Whether the guard lets through the commands that change flash or the FOTA state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Writes {
    Allowed,
    /// For a dry run: only queries go through.
    Refused,
}

/// A RACE link that refuses, before sending, every command outside the update sequence or its range.
pub struct Guard<'a, L> {
    link: &'a mut L,
    pack_len: usize,
    writes: Writes,
    partition: Option<Partition>,
}

impl<'a, L: Link<Packet>> Guard<'a, L> {
    /// Guards `link` for uploading a pack of `pack_len` bytes.
    pub fn new(link: &'a mut L, pack_len: usize, writes: Writes) -> Self {
        Guard {
            link,
            pack_len,
            writes,
            partition: None,
        }
    }

    /// The FOTA partition the headphones reported, once they have.
    pub fn partition(&self) -> Option<Partition> {
        self.partition
    }

    fn check(&self, command: &Packet) -> Result<(), String> {
        let p = &command.payload;
        // Commands without an address must carry exactly what the app sends.
        let exactly = |expected: Packet| match *p == expected.payload {
            true => Ok(()),
            false => Err(format!(
                "the app sends {}, not {}",
                hex(&expected.payload),
                hex(p)
            )),
        };
        match command.id {
            fota::READ_NV_KEY => exactly(fota::read_chip_name()),
            fota::START => exactly(fota::start()),
            fota::TRANSMIT_INTERVAL => exactly(fota::transmit_interval()),
            fota::PARTITION_INFO => exactly(fota::partition_info()),
            fota::QUERY_STATE => exactly(fota::query_state()),
            fota::ACTIVE_FOTA => exactly(fota::active_fota()),
            fota::ERASE_STATUS | fota::COMPARE => {
                if p.len() != 10 {
                    return Err(format!("expected 10 bytes, got {}", p.len()));
                }
                self.inside(p[0], u32_at(p, 2), u32_at(p, 6) as u64)
            }
            fota::START_TRANSACTION => self
                .writable()
                .and_then(|()| exactly(fota::start_transaction())),
            fota::COMMIT => self.writable().and_then(|()| exactly(fota::commit())),
            fota::WRITE_STATE => {
                self.writable()?;
                let states = [
                    fota::STATE_ERASING,
                    fota::STATE_ERASED,
                    fota::STATE_PROGRAMMING,
                    fota::STATE_CHECKED,
                ];
                match states.iter().find(|&&s| fota::write_state(s).payload == *p) {
                    Some(_) => Ok(()),
                    None => Err(format!("{} is not a state the app writes", hex(p))),
                }
            }
            fota::CHECK_INTEGRITY => {
                self.writable()?;
                let storage = *p.last().ok_or("expected the storage type")?;
                self.allowed(storage)?;
                exactly(fota::check_integrity(storage))
            }
            fota::ERASE => {
                self.writable()?;
                if p.len() != 9 {
                    return Err(format!("expected 9 bytes, got {}", p.len()));
                }
                let (length, address) = (u32_at(p, 1), u32_at(p, 5));
                if length as usize != BLOCK_SIZE || !(address as usize).is_multiple_of(BLOCK_SIZE) {
                    return Err(format!(
                        "an erase must be one 4 KB block on a 4 KB boundary, not {length:#x} bytes at \
                         {address:#x}"
                    ));
                }
                self.inside(p[0], address, length as u64)
            }
            fota::PAGE_PROGRAM => {
                self.writable()?;
                const ENTRY: usize = 1 + 4 + PAGE_SIZE;
                let pages = *p.get(1).ok_or("expected a page count")? as usize;
                if pages == 0 || p.len() != 2 + pages * ENTRY {
                    return Err(format!("{} bytes do not hold {pages} pages", p.len()));
                }
                for entry in p[2..].chunks(ENTRY) {
                    self.inside(p[0], u32_at(entry, 1), PAGE_SIZE as u64)?;
                }
                Ok(())
            }
            _ => Err("not a step of the voice-pack update".into()),
        }
    }

    fn writable(&self) -> Result<(), String> {
        match self.writes {
            Writes::Allowed => Ok(()),
            Writes::Refused => Err("this is a dry run, which changes nothing".into()),
        }
    }

    /// Checks that `length` bytes at `address` on `storage` lie in [`Self::allowed`].
    fn inside(&self, storage: u8, address: u32, length: u64) -> Result<(), String> {
        let allowed = self.allowed(storage)?;
        let range = address as u64..address as u64 + length;
        if range.start < allowed.start || range.end > allowed.end {
            return Err(format!(
                "{:#x}..{:#x} lies outside {:#x}..{:#x}, the pack rounded up to 4 KB from the start of \
                 the FOTA partition",
                range.start, range.end, allowed.start, allowed.end
            ));
        }
        Ok(())
    }

    /// The addresses that may be erased or written: the pack, rounded up to 4 KB, from the start of
    /// the reported FOTA partition, on its storage.
    fn allowed(&self, storage: u8) -> Result<Range<u64>, String> {
        let partition = self
            .partition
            .ok_or("the headphones have not reported their FOTA partition yet")?;
        if storage != partition.storage {
            return Err(format!(
                "storage {storage:#04x} is not the FOTA partition's {:#04x}",
                partition.storage
            ));
        }
        let size = fota::rounded(self.pack_len) as u64;
        if size > partition.length as u64 {
            return Err(format!(
                "the pack takes {size:#x} bytes rounded up to 4 KB, more than the FOTA partition's \
                 {:#x}",
                partition.length
            ));
        }
        let start = partition.address as u64;
        Ok(start..start + size)
    }
}

impl<L: Link<Packet>> Link<Packet> for Guard<'_, L> {
    fn request(&mut self, command: &Packet) -> io::Result<Packet> {
        self.check(command).map_err(|why| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("refused to send RACE command {:#06x}: {why}", command.id),
            )
        })?;
        let reply = self.link.request(command)?;
        if command.id == fota::PARTITION_INFO
            && reply.kind == race::RESPONSE
            && reply.payload.first() == Some(&0)
        {
            let reported = Partition::parse(&reply)?;
            match self.partition {
                None => self.partition = Some(reported),
                Some(known) if known != reported => {
                    return Err(io::Error::other(format!(
                        "the headphones first reported the FOTA partition as {known:?}, now as {reported:?}"
                    )));
                }
                Some(_) => {}
            }
        }
        Ok(reply)
    }
}
