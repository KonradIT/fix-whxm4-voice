mod common;

use std::io;

use common::FakeTransport;
use fix_whxm4_voice::link::Link;
use fix_whxm4_voice::mdr::{self, Decoder, Frame, MdrLink, Message};

fn frame(data_type: u8, seq: u8, payload: &[u8]) -> Frame {
    Frame {
        data_type,
        seq,
        payload: payload.to_vec(),
    }
}

#[test]
fn a_frame_is_marked_counted_and_summed() {
    // Type 0x0E, seq 0, length 3, payload 46 01 02, checksum 0x0E + 3 + 0x46 + 1 + 2 = 0x5A.
    let bytes = mdr::encode(&frame(0x0E, 0, &[0x46, 0x01, 0x02]));
    assert_eq!(
        bytes,
        [
            0x3E, 0x0E, 0x00, 0x00, 0x00, 0x00, 0x03, 0x46, 0x01, 0x02, 0x5A, 0x3C
        ]
    );
}

#[test]
fn an_ack_has_no_payload() {
    let bytes = mdr::encode(&frame(mdr::ACK, 1, &[]));
    assert_eq!(
        bytes,
        [0x3E, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x02, 0x3C]
    );
}

#[test]
fn marker_bytes_inside_a_frame_are_escaped() {
    let bytes = mdr::encode(&frame(0x0E, 0, &[0x3C, 0x3D, 0x3E]));
    assert_eq!(&bytes[7..13], [0x3D, 0x2C, 0x3D, 0x2D, 0x3D, 0x2E]);
    assert_eq!(bytes.iter().filter(|&&b| b == 0x3E).count(), 1);
    assert_eq!(bytes.iter().filter(|&&b| b == 0x3C).count(), 1);
}

#[test]
fn decoding_reverses_encoding_across_split_reads() {
    let sent = [
        frame(0x0E, 1, &[0x3C, 0x3D, 0x3E, 0x47]),
        frame(mdr::ACK, 0, &[]),
    ];
    let bytes: Vec<u8> = sent.iter().flat_map(mdr::encode).collect();
    let mut decoder = Decoder::default();
    let mut received = Vec::new();
    for chunk in bytes.chunks(3) {
        received.extend(decoder.push(chunk).unwrap());
    }
    assert_eq!(received, sent);
}

#[test]
fn a_frame_with_a_wrong_checksum_is_rejected() {
    let mut bytes = mdr::encode(&frame(0x0E, 0, &[0x47, 0x01]));
    let sum = bytes.len() - 2;
    bytes[sum] ^= 1;
    let err = Decoder::default().push(&bytes).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidData);
}

#[test]
fn a_request_is_acknowledged_and_answered_by_its_reply() {
    let reply = frame(0x0E, 0, &[0x47, 0x01, 0x02, 0x01]);
    let transport = FakeTransport::replying(vec![
        mdr::encode(&frame(mdr::ACK, 1, &[])),
        mdr::encode(&reply),
    ]);
    let mut link = MdrLink::new(transport);

    let answer = link
        .request(&Message {
            data_type: 0x0E,
            payload: vec![0x46, 0x01, 0x02],
        })
        .unwrap();

    assert_eq!(
        answer,
        Message {
            data_type: 0x0E,
            payload: vec![0x47, 0x01, 0x02, 0x01]
        }
    );
    let sent = link.into_transport().sent;
    assert_eq!(sent[0], mdr::encode(&frame(0x0E, 0, &[0x46, 0x01, 0x02])));
    // The reply had seq 0, so its ACK carries 1.
    assert_eq!(sent[1], mdr::encode(&frame(mdr::ACK, 1, &[])));
}

#[test]
fn the_next_request_uses_the_sequence_number_of_the_last_ack() {
    let transport = FakeTransport::replying(vec![
        mdr::encode(&frame(mdr::ACK, 1, &[])),
        mdr::encode(&frame(0x0C, 0, &[0x01, 0x00, 0x70, 0x00])),
        mdr::encode(&frame(mdr::ACK, 0, &[])),
        mdr::encode(&frame(0x0E, 1, &[0x41, 0x01])),
    ]);
    let mut link = MdrLink::new(transport);
    link.request(&Message {
        data_type: 0x0C,
        payload: vec![0x00, 0x00],
    })
    .unwrap();
    link.request(&Message {
        data_type: 0x0E,
        payload: vec![0x40, 0x01],
    })
    .unwrap();

    let sent = link.into_transport().sent;
    assert_eq!(sent[2], mdr::encode(&frame(0x0E, 1, &[0x40, 0x01])));
}

#[test]
fn messages_that_dont_answer_the_request_are_acknowledged_and_skipped() {
    let transport = FakeTransport::replying(vec![
        mdr::encode(&frame(mdr::ACK, 1, &[])),
        // A notification, and a reply to another query of the same command.
        mdr::encode(&frame(0x0E, 0, &[0x49, 0x01, 0x02, 0x00])),
        mdr::encode(&frame(0x0E, 1, &[0x47, 0x01, 0x04, 0x01])),
        mdr::encode(&frame(0x0E, 0, &[0x47, 0x01, 0x02, 0x03])),
    ]);
    let mut link = MdrLink::new(transport);

    let answer = link
        .request(&Message {
            data_type: 0x0E,
            payload: vec![0x46, 0x01, 0x02],
        })
        .unwrap();

    assert_eq!(answer.payload, [0x47, 0x01, 0x02, 0x03]);
    assert_eq!(link.into_transport().sent.len(), 4);
}

#[test]
fn a_request_without_a_reply_times_out() {
    let transport = FakeTransport::replying(vec![mdr::encode(&frame(mdr::ACK, 1, &[]))]);
    let mut link = MdrLink::new(transport);
    let err = link
        .request(&Message {
            data_type: 0x0C,
            payload: vec![0x04, 0x01],
        })
        .unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::TimedOut);
}

#[test]
fn an_ack_that_arrives_after_the_reply_still_sets_the_next_sequence_number() {
    let transport = FakeTransport::replying(vec![
        mdr::encode(&frame(0x0E, 0, &[0x47, 0x01, 0x02, 0x01])),
        mdr::encode(&frame(mdr::ACK, 1, &[])),
        mdr::encode(&frame(mdr::ACK, 0, &[])),
        mdr::encode(&frame(0x0E, 1, &[0x43, 0x01, 0x02, 0x01])),
    ]);
    let mut link = MdrLink::new(transport);
    link.request(&Message {
        data_type: 0x0E,
        payload: vec![0x46, 0x01, 0x02],
    })
    .unwrap();
    link.request(&Message {
        data_type: 0x0E,
        payload: vec![0x42, 0x01, 0x02],
    })
    .unwrap();

    let sent = link.into_transport().sent;
    assert_eq!(sent[2], mdr::encode(&frame(0x0E, 1, &[0x42, 0x01, 0x02])));
}
