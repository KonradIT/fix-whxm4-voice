mod common;

use std::io;
use std::time::Duration;

use common::FakeTransport;
use fix_whxm4_voice::fota;
use fix_whxm4_voice::link::{Link, REPLY_TIMEOUT};
use fix_whxm4_voice::race::{self, Decoder, Packet, RaceLink};

fn packet(head: u8, kind: u8, id: u16, payload: &[u8]) -> Packet {
    Packet {
        head,
        kind,
        id,
        payload: payload.to_vec(),
    }
}

#[test]
fn a_packet_is_head_type_length_id_payload() {
    let bytes = race::encode(&packet(
        0x05,
        race::COMMAND,
        0x0A00,
        &[0x02, 0x10, 0xE8, 0x03],
    ));
    assert_eq!(
        bytes,
        [0x05, 0x5A, 0x06, 0x00, 0x00, 0x0A, 0x02, 0x10, 0xE8, 0x03]
    );
}

#[test]
fn decoding_reassembles_packets_split_across_reads() {
    let sent = [
        packet(0x05, race::RESPONSE, 0x0A00, &[0x00, 0x00, 0x00]),
        packet(0x15, race::INDICATION, 0x1C08, &[0x00]),
    ];
    let bytes: Vec<u8> = sent.iter().flat_map(race::encode).collect();
    let mut decoder = Decoder::default();
    let mut received = Vec::new();
    for chunk in bytes.chunks(5) {
        received.extend(decoder.push(chunk).unwrap());
    }
    assert_eq!(received, sent);
}

#[test]
fn the_probes_chip_name_reply_decodes() {
    let packets = Decoder::default()
        .push(&[0x05, 0x5B, 0x05, 0x00, 0x00, 0x0A, 0x00, 0x00, 0x00])
        .unwrap();
    assert_eq!(
        packets,
        [packet(0x05, race::RESPONSE, 0x0A00, &[0x00, 0x00, 0x00])]
    );
}

#[test]
fn a_request_returns_the_reply_with_its_id_and_skips_others() {
    let transport = FakeTransport::replying(vec![
        race::encode(&packet(0x05, race::COMMAND, 0x0A00, &[])),
        race::encode(&packet(0x05, race::RESPONSE, 0x1C04, &[0x00])),
        race::encode(&packet(0x05, race::RESPONSE, 0x0A00, &[0x00, 0x00, 0x00])),
    ]);
    let mut link = RaceLink::new(transport);
    let command = packet(0x05, race::COMMAND, 0x0A00, &[0x02, 0x10, 0xE8, 0x03]);

    let reply = link.request(&command).unwrap();

    assert_eq!(
        reply,
        packet(0x05, race::RESPONSE, 0x0A00, &[0x00, 0x00, 0x00])
    );
    assert_eq!(link.into_transport().sent, [race::encode(&command)]);
}

#[test]
fn a_request_without_a_reply_times_out() {
    let mut link = RaceLink::new(FakeTransport::replying(Vec::new()));
    let err = link
        .request(&packet(0x05, race::COMMAND, 0x0A00, &[]))
        .unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::TimedOut);
}

#[test]
fn a_request_completed_by_an_indication_skips_the_response_before_it() {
    let transport = FakeTransport::replying(vec![
        race::encode(&packet(0x15, race::RESPONSE, 0x0433, &[0x00])),
        race::encode(&packet(0x15, race::INDICATION, 0x0433, &[0x00, 0x00, 0x01])),
    ]);
    let mut link = RaceLink::new(transport);

    let reply = link
        .request(&packet(0x15, race::COMMAND, 0x0433, &[]))
        .unwrap();

    assert_eq!(
        reply,
        packet(0x15, race::INDICATION, 0x0433, &[0x00, 0x00, 0x01])
    );
}

#[test]
fn a_refusing_response_completes_a_request_that_awaits_an_indication() {
    let transport = FakeTransport::replying(vec![race::encode(&packet(
        0x15,
        race::RESPONSE,
        0x0404,
        &[0x0A],
    ))]);
    let mut link = RaceLink::new(transport);

    let reply = link
        .request(&packet(0x15, race::COMMAND, 0x0404, &[]))
        .unwrap();

    assert_eq!(reply.payload, [0x0A]);
}

#[test]
fn a_cancel_from_the_headphones_fails_the_request_with_its_reason() {
    // What the headphones sent for 0x1C00 outside language update mode, in ticket 07's probe.
    let transport = FakeTransport::replying(vec![vec![
        0x15, 0x5A, 0x05, 0x00, 0x03, 0x1C, 0x00, 0x80, 0x04,
    ]]);
    let mut link = RaceLink::new(transport);

    let err = link
        .request(&packet(0x15, race::COMMAND, 0x1C00, &[0x00]))
        .unwrap_err();

    assert!(err.to_string().contains("not allowed"), "{err}");
}

#[test]
fn the_install_command_waits_as_long_as_the_app_and_other_commands_the_usual_time() {
    let mut link = RaceLink::new(FakeTransport::replying(vec![race::encode(&packet(
        0x15,
        race::RESPONSE,
        fota::QUERY_STATE,
        &[0x00, 0xFF, 0xFF],
    ))]));
    link.request(&fota::query_state()).unwrap();
    let _ = link.request(&fota::commit());

    let transport = link.into_transport();
    assert_eq!(transport.timeouts[0], REPLY_TIMEOUT);
    assert!(
        transport.timeouts[1..]
            .iter()
            .all(|&t| t == fota::COMMIT_TIMEOUT)
    );
    assert_eq!(fota::COMMIT_TIMEOUT, Duration::from_secs(15));
}
