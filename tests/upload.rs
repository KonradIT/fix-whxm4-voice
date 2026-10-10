mod common;

use std::cell::RefCell;
use std::io;
use std::rc::Rc;

use common::built_pack;
use fix_whxm4_voice::build::BuiltPack;
use fix_whxm4_voice::fota::{self, Partition, u32_at};
use fix_whxm4_voice::link::Link;
use fix_whxm4_voice::mdr::Message;
use fix_whxm4_voice::race::Packet;
use fix_whxm4_voice::upload::{self, Mode};
use sha2::{Digest, Sha256};

const PARTITION: Partition = Partition {
    id: 0x01,
    storage: 0x02,
    address: 0x0810_0000,
    length: 0x0010_0000,
};

/// The headphones as the two links see them: language update mode, the FOTA state and the FOTA
/// partition's flash. Flash behaves like NOR flash: erasing sets bytes to 0xFF, programming can
/// only clear bits.
struct Device {
    /// Every command that reached the headphones, as `MDR on`, `MDR off`, `0x1c08` or, for a state
    /// write, `0x1c06 0200`.
    log: Vec<String>,
    update_mode: bool,
    state: u16,
    partition: Partition,
    flash: Vec<u8>,
    /// Makes the integrity check fail whatever flash holds.
    fail_integrity: bool,
    /// The pack the integrity check expects at the start of the partition.
    expected: Vec<u8>,
}

impl Device {
    fn new(flash_byte: u8) -> Self {
        Device {
            log: Vec::new(),
            update_mode: false,
            state: fota::STATE_NONE,
            partition: PARTITION,
            flash: vec![flash_byte; PARTITION.length as usize],
            fail_integrity: false,
            expected: built_pack(),
        }
    }

    /// The offset into flash of `length` bytes at `address`, which must lie inside the partition.
    fn offset(&self, address: u32, length: usize) -> usize {
        let offset = (address - self.partition.address) as usize;
        assert!(
            offset + length <= self.flash.len(),
            "{address:#x}+{length:#x} outside"
        );
        offset
    }

    fn race(&mut self, command: &Packet) -> io::Result<Packet> {
        let p = &command.payload;
        self.log.push(match command.id {
            fota::WRITE_STATE => format!("{:#06x} {:02x}{:02x}", command.id, p[1], p[0]),
            id => format!("{id:#06x}"),
        });
        if !self.update_mode && command.id != fota::READ_NV_KEY {
            return Err(io::Error::other("cancelled: not allowed"));
        }
        let mut reply = vec![0x00];
        match command.id {
            fota::READ_NV_KEY => reply.extend([0x00, 0x00]),
            fota::TRANSMIT_INTERVAL => reply.extend([0x00, 0x00, 0x64, 0x00]),
            fota::PARTITION_INFO => {
                reply.extend([self.partition.id, self.partition.storage]);
                reply.extend(self.partition.address.to_le_bytes());
                reply.extend(self.partition.length.to_le_bytes());
            }
            fota::QUERY_STATE => reply.extend(self.state.to_le_bytes()),
            fota::ACTIVE_FOTA => reply.push(0x00),
            fota::ERASE_STATUS => {
                let (address, length) = (u32_at(p, 2), u32_at(p, 6) as usize);
                let offset = self.offset(address, length);
                let blocks: Vec<bool> = self.flash[offset..offset + length]
                    .chunks(fota::BLOCK_SIZE)
                    .map(|block| block.iter().all(|&b| b == 0xFF))
                    .collect();
                let mut bitmap = vec![0u8; blocks.len().div_ceil(8)];
                for (k, &erased) in blocks.iter().enumerate() {
                    if erased {
                        bitmap[k / 8] |= 0x80 >> (k % 8);
                    }
                }
                reply.extend([0x00, 0x00]);
                reply.extend(&p[2..10]);
                reply.extend((bitmap.len() as u16).to_le_bytes());
                reply.extend(bitmap);
            }
            fota::COMPARE => {
                let (address, length) = (u32_at(p, 2), u32_at(p, 6) as usize);
                let offset = self.offset(address, length);
                reply.extend([0x00, 0x00]);
                reply.extend(&p[2..10]);
                reply.extend(Sha256::digest(&self.flash[offset..offset + length]));
            }
            fota::WRITE_STATE => self.state = u16::from_le_bytes([p[0], p[1]]),
            fota::ERASE => {
                let offset = self.offset(u32_at(p, 5), fota::BLOCK_SIZE);
                self.flash[offset..offset + fota::BLOCK_SIZE].fill(0xFF);
                reply.push(0x00);
                reply.extend(&p[1..9]);
            }
            fota::PAGE_PROGRAM => {
                assert_eq!(p[1], 1, "the app sends one page per command");
                let address = u32_at(p, 3);
                let page = &p[7..];
                assert_eq!(p[2], fota::page_crc(page), "page CRC at {address:#x}");
                let offset = self.offset(address, page.len());
                for (byte, &new) in self.flash[offset..].iter_mut().zip(page) {
                    *byte &= new;
                }
                reply.extend([p[0], 1]);
                reply.extend(address.to_le_bytes());
            }
            fota::CHECK_INTEGRITY => {
                let staged = &self.flash[..self.expected.len()];
                if self.fail_integrity || staged != self.expected {
                    reply[0] = 0x01;
                }
                reply.extend([0x01, 0x00, p[2]]);
            }
            fota::COMMIT => {
                // The headphones restart to install the pack, which drops the link.
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "the headphones closed the RACE channel",
                ));
            }
            _ => {}
        }
        Ok(Packet {
            head: fota::HEAD,
            kind: fota::reply_kind(command.id),
            id: command.id,
            payload: reply,
        })
    }

    fn mdr(&mut self, command: &Message) -> io::Result<Message> {
        let [0x44, 0x01, 0x02, value] = command.payload[..] else {
            panic!("unexpected MDR command {:02x?}", command.payload);
        };
        self.update_mode = value == 0x00;
        self.log.push(format!(
            "MDR {}",
            if self.update_mode { "on" } else { "off" }
        ));
        Ok(Message {
            data_type: command.data_type,
            payload: vec![0x45, 0x01, 0x02, value],
        })
    }
}

struct Race(Rc<RefCell<Device>>);
struct Mdr(Rc<RefCell<Device>>);

impl Link<Packet> for Race {
    fn request(&mut self, command: &Packet) -> io::Result<Packet> {
        self.0.borrow_mut().race(command)
    }
}

impl Link<Message> for Mdr {
    fn request(&mut self, command: &Message) -> io::Result<Message> {
        self.0.borrow_mut().mdr(command)
    }
}

/// Runs the upload of the built pack in `mode` against `device`, and returns the device again.
fn upload(device: Device, mode: Mode) -> (io::Result<upload::Plan>, Device) {
    let device = Rc::new(RefCell::new(device));
    let pack = BuiltPack::verify(built_pack()).unwrap();
    let result = upload::run(
        &mut Mdr(device.clone()),
        &mut Race(device.clone()),
        &pack,
        mode,
    );
    let device = Rc::try_unwrap(device).ok().unwrap().into_inner();
    (result, device)
}

/// The log with repeated commands collapsed into one, such as one `0x0404` for every erase.
fn steps(device: &Device) -> Vec<&str> {
    let mut steps: Vec<&str> = device.log.iter().map(String::as_str).collect();
    steps.dedup();
    steps
}

fn count(device: &Device, step: &str) -> usize {
    device.log.iter().filter(|s| *s == step).count()
}

const WRITES: [&str; 9] = [
    "0x1c0a",
    "0x1c06 0200",
    "0x1c06 0201",
    "0x1c06 0210",
    "0x1c06 0211",
    "0x0404",
    "0x0402",
    "0x1c01",
    "0x1c02",
];

#[test]
fn the_built_pack_spans_several_blocks() {
    // The tests below rely on this, so that stage skips can apply to some blocks only.
    assert!(built_pack().len() > 3 * fota::BLOCK_SIZE);
}

#[test]
fn an_upload_sends_the_apps_steps_in_the_apps_order() {
    let (result, device) = upload(Device::new(0x00), Mode::Upload);

    result.unwrap();
    assert_eq!(
        steps(&device),
        [
            "MDR on",
            "0x0a00",
            "0x1c08",
            "0x1c1c",
            "0x1c00",
            "0x1c04",
            "0x1c19",
            "0x0433",
            "0x0431",
            "0x1c0a",
            "0x1c06 0200",
            "0x0404",
            "0x1c06 0201",
            "0x1c06 0210",
            "0x0402",
            "0x1c01",
            "0x1c06 0211",
            "0x1c04",
            "MDR off",
            "MDR on",
            "0x1c02",
        ]
    );
}

#[test]
fn an_upload_leaves_the_pack_at_the_start_of_the_partition() {
    let (result, device) = upload(Device::new(0x00), Mode::Upload);

    result.unwrap();
    let pack = built_pack();
    assert_eq!(device.flash[..pack.len()], pack);
    let blocks = pack.len().div_ceil(fota::BLOCK_SIZE);
    assert_eq!(count(&device, "0x0404"), blocks);
    // The flash after the pack's last 4 KB block is untouched.
    assert!(
        device.flash[blocks * fota::BLOCK_SIZE..]
            .iter()
            .all(|&b| b == 0x00)
    );
}

#[test]
fn already_erased_blocks_are_neither_compared_nor_erased() {
    let (result, device) = upload(Device::new(0xFF), Mode::Upload);

    result.unwrap();
    for skipped in ["0x0431", "0x1c06 0200", "0x0404"] {
        assert_eq!(count(&device, skipped), 0, "{skipped}");
    }
    for sent in ["0x1c0a", "0x1c06 0201", "0x1c06 0210", "0x0402", "0x1c01"] {
        assert!(count(&device, sent) > 0, "{sent}");
    }
}

#[test]
fn blocks_already_holding_the_pack_are_not_erased_or_programmed() {
    let mut device = Device::new(0x00);
    let pack = built_pack();
    device.flash[..pack.len()].copy_from_slice(&pack);
    // One byte in the second block differs.
    device.flash[fota::BLOCK_SIZE + 5] ^= 0x01;

    let (result, device) = upload(device, Mode::Upload);

    result.unwrap();
    assert_eq!(count(&device, "0x0404"), 1);
    assert_eq!(count(&device, "0x0402"), fota::BLOCK_SIZE / fota::PAGE_SIZE);
    assert_eq!(device.flash[..pack.len()], pack);
}

#[test]
fn an_upload_onto_the_pack_itself_only_checks_and_commits() {
    let mut device = Device::new(0x00);
    let pack = built_pack();
    device.flash[..pack.len()].copy_from_slice(&pack);

    let (result, device) = upload(device, Mode::Upload);

    result.unwrap();
    for skipped in [
        "0x1c06 0200",
        "0x0404",
        "0x1c06 0201",
        "0x1c06 0210",
        "0x0402",
    ] {
        assert_eq!(count(&device, skipped), 0, "{skipped}");
    }
    for sent in ["0x1c0a", "0x1c01", "0x1c06 0211", "0x1c02"] {
        assert_eq!(count(&device, sent), 1, "{sent}");
    }
}

#[test]
fn a_failed_checksum_check_is_a_failure_and_nothing_is_committed() {
    let mut device = Device::new(0x00);
    device.fail_integrity = true;

    let (result, device) = upload(device, Mode::Upload);

    let err = result.unwrap_err().to_string();
    assert!(err.contains("checksum check"), "{err}");
    assert_eq!(count(&device, "0x1c02"), 0);
    assert_eq!(count(&device, "0x1c06 0211"), 0);
    assert_eq!(device.log.last().unwrap(), "MDR off");
}

#[test]
fn a_dry_run_changes_nothing_and_switches_update_mode_off_again() {
    let (result, device) = upload(Device::new(0x00), Mode::DryRun);

    result.unwrap();
    for write in WRITES {
        assert_eq!(count(&device, write), 0, "{write}");
    }
    assert!(device.flash.iter().all(|&b| b == 0x00));
    assert_eq!(device.log.first().unwrap(), "MDR on");
    assert_eq!(device.log.last().unwrap(), "MDR off");
    assert!(!device.update_mode);
}

#[test]
fn a_dry_run_reports_the_partition_the_planned_ranges_and_the_page_count() {
    let mut device = Device::new(0x00);
    let pack = built_pack();
    device.flash[..pack.len()].copy_from_slice(&pack);
    device.flash[fota::BLOCK_SIZE + 5] ^= 0x01;

    let (result, _) = upload(device, Mode::DryRun);

    let plan = result.unwrap();
    assert_eq!(plan.partition, PARTITION);
    assert_eq!(plan.pages(), fota::BLOCK_SIZE / fota::PAGE_SIZE);
    let report = plan.to_string();
    let end = PARTITION.address as usize + pack.len().div_ceil(fota::BLOCK_SIZE) * fota::BLOCK_SIZE;
    for expected in [
        "0x08100000".to_string(),
        format!("{end:#010x}"),
        "0x08101000..0x08102000".to_string(),
        "16 pages".to_string(),
    ] {
        assert!(report.contains(&expected), "{expected} not in:\n{report}");
    }
}

#[test]
fn a_partition_too_small_for_the_pack_is_refused_before_any_range_is_sent() {
    let mut device = Device::new(0x00);
    device.partition.length = 0x1000;

    let (result, device) = upload(device, Mode::Upload);

    let err = result.unwrap_err().to_string();
    assert!(err.contains("FOTA partition"), "{err}");
    for range in ["0x0433", "0x0431", "0x0404", "0x0402"] {
        assert_eq!(count(&device, range), 0, "{range}");
    }
    assert_eq!(device.log.last().unwrap(), "MDR off");
}

#[test]
fn a_staged_image_awaiting_its_commit_is_refused_without_a_transfer() {
    let mut device = Device::new(0x00);
    device.state = fota::STATE_CHECKED;

    let (result, device) = upload(device, Mode::Upload);

    let err = result.unwrap_err().to_string();
    assert!(err.contains("never committed"), "{err}");
    assert_eq!(
        steps(&device),
        [
            "MDR on", "0x0a00", "0x1c08", "0x1c1c", "0x1c00", "0x1c04", "MDR off"
        ]
    );
}

#[test]
fn a_state_missing_from_the_protocol_notes_is_reported_as_such() {
    let mut device = Device::new(0xFF);
    // What the user's headphones reported in the hand run of ticket 09.
    device.state = 0x0101;

    let (result, _) = upload(device, Mode::DryRun);

    let report = result.unwrap().to_string();
    assert!(
        report.contains("0x0101 (not in the protocol notes)"),
        "{report}"
    );
}
