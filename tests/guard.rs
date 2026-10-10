use std::io;

use fix_whxm4_voice::fota::{self, Partition};
use fix_whxm4_voice::guard::{Guard, Writes};
use fix_whxm4_voice::link::Link;
use fix_whxm4_voice::race::{self, Packet};

const PARTITION: Partition = Partition {
    id: 0x01,
    storage: 0x02,
    address: 0x0810_0000,
    length: 0x0001_0000,
};

/// Answers every command with success, and 0x1C00 with [`PARTITION`]. Records what reaches it.
#[derive(Default)]
struct Headphones {
    sent: Vec<Packet>,
}

impl Link<Packet> for Headphones {
    fn request(&mut self, command: &Packet) -> io::Result<Packet> {
        self.sent.push(command.clone());
        let mut payload = vec![0x00];
        if command.id == fota::PARTITION_INFO {
            payload.extend([PARTITION.id, PARTITION.storage]);
            payload.extend(PARTITION.address.to_le_bytes());
            payload.extend(PARTITION.length.to_le_bytes());
        }
        Ok(Packet {
            head: 0x15,
            kind: fota::reply_kind(command.id),
            id: command.id,
            payload,
        })
    }
}

fn ids(sent: &[Packet]) -> Vec<u16> {
    sent.iter().map(|p| p.id).collect()
}

const PAGE: [u8; 256] = [0x11; 256];

/// A pack of 0x1800 bytes, so the guard allows 0x2000 bytes from the partition address.
const PACK_LEN: usize = 0x1800;

fn refused(result: io::Result<Packet>) -> String {
    let err = result.expect_err("the guard let the command through");
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied, "{err}");
    err.to_string()
}

#[test]
fn erase_and_write_are_refused_until_the_partition_is_reported() {
    let mut headphones = Headphones::default();
    let mut guard = Guard::new(&mut headphones, PACK_LEN, Writes::Allowed);

    refused(guard.request(&fota::erase(0x02, PARTITION.address)));
    refused(guard.request(&fota::page_program(0x02, PARTITION.address, &PAGE)));

    assert!(headphones.sent.is_empty());
}

#[test]
fn erase_and_write_inside_the_pack_rounded_to_4_kb_go_through() {
    let mut headphones = Headphones::default();
    let mut guard = Guard::new(&mut headphones, PACK_LEN, Writes::Allowed);
    guard.request(&fota::partition_info()).unwrap();

    guard
        .request(&fota::erase(0x02, PARTITION.address + 0x1000))
        .unwrap();
    guard
        .request(&fota::page_program(0x02, PARTITION.address + 0x1F00, &PAGE))
        .unwrap();

    assert_eq!(guard.partition(), Some(PARTITION));
    assert_eq!(ids(&headphones.sent), [0x1C00, 0x0404, 0x0402]);
}

#[test]
fn erase_and_write_outside_the_pack_or_partition_are_refused_before_sending() {
    let mut headphones = Headphones::default();
    let mut guard = Guard::new(&mut headphones, PACK_LEN, Writes::Allowed);
    guard.request(&fota::partition_info()).unwrap();

    // Beyond the pack rounded up to 4 KB, though still inside the partition.
    refused(guard.request(&fota::erase(0x02, PARTITION.address + 0x2000)));
    refused(guard.request(&fota::page_program(0x02, PARTITION.address + 0x2000, &PAGE)));
    // Below the partition.
    refused(guard.request(&fota::erase(0x02, PARTITION.address - 0x1000)));
    refused(guard.request(&fota::page_program(0x02, PARTITION.address - 0x100, &PAGE)));
    // A page straddling the end, and a block not on a 4 KB boundary.
    refused(guard.request(&fota::page_program(0x02, PARTITION.address + 0x1F80, &PAGE)));
    refused(guard.request(&fota::erase(0x02, PARTITION.address + 0x800)));
    // Another storage than the partition's.
    refused(guard.request(&fota::erase(0x00, PARTITION.address)));
    // Range queries obey the same bounds.
    refused(guard.request(&fota::erase_status(0x02, PARTITION.address, 0x3000)));
    refused(guard.request(&fota::compare(0x02, PARTITION.address + 0x1000, 0x1001)));

    assert_eq!(ids(&headphones.sent), [0x1C00]);
}

#[test]
fn a_page_program_with_several_pages_is_checked_page_by_page() {
    let mut headphones = Headphones::default();
    let mut guard = Guard::new(&mut headphones, PACK_LEN, Writes::Allowed);
    guard.request(&fota::partition_info()).unwrap();
    let inside = fota::page_program(0x02, PARTITION.address, &PAGE);
    let outside = fota::page_program(0x02, PARTITION.address + 0x2000, &PAGE);
    let mut two = inside.clone();
    two.payload[1] = 2;
    two.payload.extend(&outside.payload[2..]);

    refused(guard.request(&two));
}

#[test]
fn a_pack_larger_than_the_partition_is_refused() {
    let mut headphones = Headphones::default();
    let mut guard = Guard::new(&mut headphones, 0x1_0001, Writes::Allowed);
    guard.request(&fota::partition_info()).unwrap();

    let message = refused(guard.request(&fota::erase(0x02, PARTITION.address)));

    assert!(message.contains("FOTA partition"), "{message}");
}

#[test]
fn commands_outside_the_update_sequence_are_refused() {
    let mut headphones = Headphones::default();
    let mut guard = Guard::new(&mut headphones, PACK_LEN, Writes::Allowed);
    // A RACE flash page read, which ticket 07 found blocked anyway.
    let read = Packet {
        head: 0x05,
        kind: race::COMMAND,
        id: 0x0403,
        payload: vec![0; 6],
    };

    refused(guard.request(&read));

    assert!(headphones.sent.is_empty());
}

#[test]
fn with_writes_refused_nothing_that_changes_flash_or_state_goes_through() {
    let mut headphones = Headphones::default();
    let mut guard = Guard::new(&mut headphones, PACK_LEN, Writes::Refused);
    for query in [
        fota::read_chip_name(),
        fota::start(),
        fota::transmit_interval(),
        fota::partition_info(),
        fota::query_state(),
        fota::active_fota(),
        fota::erase_status(0x02, PARTITION.address, 0x2000),
        fota::compare(0x02, PARTITION.address, 0x1000),
    ] {
        guard.request(&query).unwrap();
    }

    for write in [
        fota::start_transaction(),
        fota::write_state(fota::STATE_ERASING),
        fota::erase(0x02, PARTITION.address),
        fota::page_program(0x02, PARTITION.address, &PAGE),
        fota::check_integrity(0x02),
        fota::commit(),
    ] {
        refused(guard.request(&write));
    }

    assert_eq!(
        ids(&headphones.sent),
        [
            0x0A00, 0x1C08, 0x1C1C, 0x1C00, 0x1C04, 0x1C19, 0x0433, 0x0431
        ]
    );
}

#[test]
fn commands_without_an_address_must_carry_what_the_app_sends() {
    let mut headphones = Headphones::default();
    let mut guard = Guard::new(&mut headphones, PACK_LEN, Writes::Allowed);
    let mut tws_start = fota::start();
    tws_start.payload = vec![0x03];
    let mut odd_state = fota::write_state(fota::STATE_ERASING);
    odd_state.payload = vec![0x34, 0x12];

    refused(guard.request(&tws_start));
    refused(guard.request(&odd_state));
    // The integrity check of a storage other than the reported partition's.
    guard.request(&fota::partition_info()).unwrap();
    refused(guard.request(&fota::check_integrity(0x00)));

    assert_eq!(ids(&headphones.sent), [0x1C00]);
}
