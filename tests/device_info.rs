use std::io;

use fix_whxm4_voice::device_info::{self, DeviceInfo};
use fix_whxm4_voice::link::Link;
use fix_whxm4_voice::mdr::Message;
use fix_whxm4_voice::race::{self, Packet};

/// The read-only MDR commands `docs/upload-protocol.md` lists, as (data type, payload).
const READ_ONLY_MDR: [(u8, &[u8]); 7] = [
    (0x0C, &[0x00, 0x00]),
    (0x0C, &[0x04, 0x01]),
    (0x0C, &[0x04, 0x02]),
    (0x0E, &[0x40, 0x01]),
    (0x0E, &[0x42, 0x01, 0x02]),
    (0x0E, &[0x46, 0x01, 0x02]),
    (0x0E, &[0x46, 0x01, 0x04]),
];

/// Answers with the replies the user's headphones gave in tickets 07 and 08, and records every
/// command. Commands it has no reply for time out.
#[derive(Default)]
struct FakeMdr {
    sent: Vec<Message>,
    silent: Vec<Vec<u8>>,
}

impl Link<Message> for FakeMdr {
    fn request(&mut self, command: &Message) -> io::Result<Message> {
        self.sent.push(command.clone());
        let payload: Vec<u8> = match command.payload.as_slice() {
            p if self.silent.iter().any(|s| s == p) => Vec::new(),
            [0x00, 0x00] => vec![0x01, 0x00, 0x70, 0x00],
            [0x04, 0x01] => [&[0x05, 0x01, 10][..], b"WH-1000XM4"].concat(),
            [0x04, 0x02] => [&[0x05, 0x02, 5][..], b"3.0.1"].concat(),
            [0x40, 0x01] => [
                &[0x41, 0x01, 0x01, 0x01, 0x0F][..],
                &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 0x0D, 0x0F, 0x10, 0xF0],
            ]
            .concat(),
            [0x42, 0x01, 0x02] => vec![0x43, 0x01, 0x02, 0x01],
            [0x46, 0x01, 0x02] => vec![0x47, 0x01, 0x02, 0x01],
            [0x46, 0x01, 0x04] => [
                &[0x47, 0x01, 0x04, 0x01, 5][..],
                b"HP002",
                &[4],
                b"1234",
                &[2, 0x01, 11],
                b"VGIDLPB0401",
                &[0x03, 11],
                b"VGIDLPB0403",
            ]
            .concat(),
            _ => Vec::new(),
        };
        if payload.is_empty() {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "no reply"));
        }
        Ok(Message {
            data_type: command.data_type,
            payload,
        })
    }
}

#[derive(Default)]
struct FakeRace {
    sent: Vec<Packet>,
}

impl Link<Packet> for FakeRace {
    fn request(&mut self, command: &Packet) -> io::Result<Packet> {
        self.sent.push(command.clone());
        Ok(Packet {
            head: 0x05,
            kind: race::RESPONSE,
            id: command.id,
            payload: vec![0, 0, 0],
        })
    }
}

fn read(mdr: &mut FakeMdr) -> (DeviceInfo, FakeRace) {
    let mut race = FakeRace::default();
    let info = device_info::read(mdr, &mut race).unwrap();
    (info, race)
}

#[test]
fn only_documented_read_only_commands_are_sent() {
    let mut mdr = FakeMdr::default();
    let (_, race) = read(&mut mdr);

    for command in &mdr.sent {
        assert!(
            READ_ONLY_MDR.contains(&(command.data_type, command.payload.as_slice())),
            "not a read-only MDR command: {command:?}"
        );
    }
    assert_eq!(mdr.sent.len(), READ_ONLY_MDR.len());
    // The only RACE command is the NV-key read of the chip name.
    assert_eq!(
        race.sent,
        [Packet {
            head: 0x05,
            kind: race::COMMAND,
            id: 0x0A00,
            payload: vec![0x02, 0x10, 0xE8, 0x03],
        }]
    );
}

#[test]
fn the_replies_are_read_into_the_device_info() {
    let (info, _) = read(&mut FakeMdr::default());

    assert_eq!(info.model.as_deref(), Some("WH-1000XM4"));
    assert_eq!(info.firmware.as_deref(), Some("3.0.1"));
    assert_eq!(info.protocol_info, [0x70, 0x00]);
    assert_eq!(info.chip_name.as_deref(), Some(""));
    assert_eq!(info.language, 0x01);
    assert!(!info.update_mode);
    assert_eq!(info.supported.len(), 15);
    assert_eq!(info.supported[14], 0xF0);
    assert_eq!(info.category, "HP002");
    assert_eq!(info.serial, "1234");
    assert_eq!(
        info.packs,
        [
            (0x01, "VGIDLPB0401".to_string()),
            (0x03, "VGIDLPB0403".to_string())
        ]
    );
}

#[test]
fn update_mode_reads_on_for_status_zero() {
    struct On(FakeMdr);
    impl Link<Message> for On {
        fn request(&mut self, command: &Message) -> io::Result<Message> {
            let mut reply = self.0.request(command)?;
            if command.payload == [0x42, 0x01, 0x02] {
                reply.payload[3] = 0x00;
            }
            Ok(reply)
        }
    }
    let info = device_info::read(&mut On(FakeMdr::default()), &mut FakeRace::default()).unwrap();
    assert!(info.update_mode);
}

#[test]
fn model_and_firmware_are_optional() {
    let mut mdr = FakeMdr {
        silent: vec![vec![0x04, 0x01], vec![0x04, 0x02]],
        ..Default::default()
    };
    let (info, _) = read(&mut mdr);

    assert_eq!(info.model, None);
    assert_eq!(info.firmware, None);
    assert_eq!(info.language, 0x01);
    assert!(info.to_string().contains("not reported"));
}

#[test]
fn a_missing_language_reply_is_an_error() {
    let mut mdr = FakeMdr {
        silent: vec![vec![0x46, 0x01, 0x02]],
        ..Default::default()
    };
    let err = device_info::read(&mut mdr, &mut FakeRace::default()).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::TimedOut);
}

#[test]
fn the_report_names_languages_and_their_packs() {
    let (info, _) = read(&mut FakeMdr::default());
    let report = info.to_string();

    assert!(report.contains("WH-1000XM4"), "{report}");
    assert!(report.contains("English (0x01)"), "{report}");
    assert!(report.contains("German (0x03)"), "{report}");
    assert!(report.contains("VGIDLPB0403"), "{report}");
    assert!(report.contains("Chinese (0xf0)"), "{report}");
    assert!(report.contains("off"), "{report}");
}

#[test]
fn a_refused_chip_name_read_is_reported_as_refused() {
    struct Refusing;
    impl Link<Packet> for Refusing {
        fn request(&mut self, command: &Packet) -> io::Result<Packet> {
            Ok(Packet {
                head: 0x05,
                kind: race::RESPONSE,
                id: command.id,
                payload: vec![0x03],
            })
        }
    }
    let info = device_info::read(&mut FakeMdr::default(), &mut Refusing).unwrap();

    assert_eq!(info.chip_name, None);
    assert!(info.to_string().contains("refused"));
}
