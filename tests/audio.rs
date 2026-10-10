use fix_whxm4_voice::audio::{Convert, Ffmpeg, Format, MpegVersion, measure};

/// Frame header of a stock speech clip: MPEG-1 Layer III, 64 kbit/s, 48 kHz, mono. 192-byte frames.
const SPEECH: [u8; 4] = [0xFF, 0xFB, 0x54, 0xC4];
/// Frame header of a stock tone clip: as `SPEECH`, but 48 kbit/s. 144-byte frames.
const TONE: [u8; 4] = [0xFF, 0xFB, 0x34, 0xC4];
/// MPEG-2 Layer III, 32 kbit/s, 22050 Hz, stereo, padded. 105-byte frames.
const MPEG2_PADDED: [u8; 4] = [0xFF, 0xF3, 0x42, 0x00];

fn frame(header: [u8; 4], len: usize) -> Vec<u8> {
    let mut frame = header.to_vec();
    frame.resize(len, 0x00);
    frame
}

fn frames(header: [u8; 4], len: usize, count: usize) -> Vec<u8> {
    (0..count).flat_map(|_| frame(header, len)).collect()
}

/// An ID3v2.3 tag with `body` bytes after its 10-byte header.
fn id3v2(body: usize) -> Vec<u8> {
    let mut tag = b"ID3\x03\x00\x00".to_vec();
    tag.extend([
        (body >> 21) as u8 & 0x7F,
        (body >> 14) as u8 & 0x7F,
        (body >> 7) as u8 & 0x7F,
        body as u8 & 0x7F,
    ]);
    tag.resize(10 + body, 0x00);
    tag
}

fn error(clip: &[u8]) -> String {
    match measure(clip) {
        Ok(format) => panic!("measured {format:?}"),
        Err(e) => e.to_string(),
    }
}

const STOCK_SPEECH: Format = Format {
    version: MpegVersion::Mpeg1,
    bitrate_kbps: 64,
    sample_rate: 48_000,
    channels: 1,
};

#[test]
fn measures_a_stock_speech_clip() {
    assert_eq!(measure(&frames(SPEECH, 192, 5)).unwrap(), STOCK_SPEECH);
}

#[test]
fn measures_a_stock_tone_clip() {
    assert_eq!(measure(&frames(TONE, 144, 5)).unwrap().bitrate_kbps, 48);
}

#[test]
fn measures_mpeg2_with_padded_frames() {
    assert_eq!(
        measure(&frames(MPEG2_PADDED, 105, 3)).unwrap(),
        Format {
            version: MpegVersion::Mpeg2,
            bitrate_kbps: 32,
            sample_rate: 22_050,
            channels: 2,
        }
    );
}

#[test]
fn skips_an_id3v2_tag_at_the_start() {
    let mut clip = id3v2(1142);
    clip.extend(frames(SPEECH, 192, 5));

    assert_eq!(measure(&clip).unwrap(), STOCK_SPEECH);
}

#[test]
fn skips_an_id3v1_tag_at_the_end() {
    let mut clip = frames(SPEECH, 192, 5);
    clip.extend(b"TAG");
    clip.resize(clip.len() + 125, b' ');

    assert_eq!(measure(&clip).unwrap(), STOCK_SPEECH);
}

#[test]
fn a_clip_changing_format_between_frames_is_refused() {
    let mut clip = frames(SPEECH, 192, 2);
    clip.extend(frame(TONE, 144));

    assert!(error(&clip).contains("frame"));
}

#[test]
fn a_clip_without_frames_is_refused() {
    assert!(!error(b"").is_empty());
    assert!(!error(&id3v2(20)).is_empty());
    assert!(!error(b"RIFF\x24\x00\x00\x00WAVEfmt ").is_empty());
}

#[test]
fn a_truncated_last_frame_is_refused() {
    let clip = frames(SPEECH, 192, 3);

    assert!(!error(&clip[..clip.len() - 1]).is_empty());
}

#[test]
fn a_format_reads_as_a_short_description() {
    assert_eq!(
        STOCK_SPEECH.to_string(),
        "MPEG-1 Layer III, 48000 Hz, mono, 64 kbit/s CBR"
    );
}

#[test]
fn a_silent_clip_is_the_shortest_ffmpeg_makes_in_the_stock_format() {
    let stock = frames(SPEECH, 192, 5);

    let silence = Ffmpeg.silence(&stock).unwrap();

    assert_eq!(measure(&silence).unwrap(), STOCK_SPEECH);
    // Two frames of 1152 samples at 48 kHz: 48 ms.
    assert_eq!(silence.len(), 2 * 192);
}
