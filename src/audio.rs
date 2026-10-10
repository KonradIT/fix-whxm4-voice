//! Audio conversion: measures the format of stock clips and converts changes to it with ffmpeg.

use std::ffi::OsStr;
use std::fmt;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::invalid;

/// Length of a silent clip, in seconds.
const SILENCE_SECONDS: &str = "0.1";

/// Bitrates of MPEG-1 Layer III by bitrate index, in kbit/s. Index 0 (free format) and 15 are invalid.
const MPEG1_BITRATES: [u32; 15] = [
    0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320,
];
/// Bitrates of MPEG-2 and MPEG-2.5 Layer III by bitrate index, in kbit/s.
const MPEG2_BITRATES: [u32; 15] = [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160];
/// Sample rates of MPEG-1 by sample rate index, in Hz; MPEG-2 halves them, MPEG-2.5 quarters them.
const MPEG1_SAMPLE_RATES: [u32; 3] = [44_100, 48_000, 32_000];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MpegVersion {
    Mpeg1,
    Mpeg2,
    Mpeg25,
}

/// The format of an MP3 clip whose frames all share one format (constant bitrate, Layer III).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    pub version: MpegVersion,
    pub bitrate_kbps: u32,
    pub sample_rate: u32,
    pub channels: u8,
}

impl Format {
    /// The channel layout, as ffmpeg names it.
    fn channel_layout(&self) -> &'static str {
        if self.channels == 1 { "mono" } else { "stereo" }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let version = match self.version {
            MpegVersion::Mpeg1 => "MPEG-1",
            MpegVersion::Mpeg2 => "MPEG-2",
            MpegVersion::Mpeg25 => "MPEG-2.5",
        };
        write!(
            f,
            "{version} Layer III, {} Hz, {}, {} kbit/s CBR",
            self.sample_rate,
            self.channel_layout(),
            self.bitrate_kbps
        )
    }
}

/// Measures the format of an MP3 clip by reading every frame header. An ID3v2 tag at the start and
/// an ID3v1 tag at the end are skipped. Refuses anything but back-to-back Layer III frames of one
/// format.
pub fn measure(clip: &[u8]) -> io::Result<Format> {
    let end = if clip.len() >= 128 && clip[clip.len() - 128..].starts_with(b"TAG") {
        clip.len() - 128
    } else {
        clip.len()
    };
    let mut pos = id3v2_len(clip);
    let mut format = None;
    while pos < end {
        let (frame_format, len) = frame_header(&clip[pos..end])
            .ok_or_else(|| invalid(format!("no MP3 Layer III frame at byte {pos}")))?;
        if *format.get_or_insert(frame_format) != frame_format {
            return Err(invalid(format!(
                "the frame at byte {pos} is {frame_format}, but the first frame is {}",
                format.unwrap()
            )));
        }
        if pos + len > end {
            return Err(invalid(format!("the frame at byte {pos} is cut off")));
        }
        pos += len;
    }
    format.ok_or_else(|| invalid("no MP3 frames"))
}

/// Length of the ID3v2 tag at the start of `clip`, or 0 without one.
fn id3v2_len(clip: &[u8]) -> usize {
    if clip.len() < 10 || !clip.starts_with(b"ID3") {
        return 0;
    }
    let size = clip[6..10]
        .iter()
        .fold(0, |size, &b| size << 7 | usize::from(b & 0x7F));
    let footer = if clip[5] & 0x10 != 0 { 10 } else { 0 };
    10 + size + footer
}

/// The format and length of the Layer III frame starting `bytes`, or `None` if none starts there.
fn frame_header(bytes: &[u8]) -> Option<(Format, usize)> {
    let &[b0, b1, b2, b3, ..] = bytes else {
        return None;
    };
    if b0 != 0xFF || b1 & 0xE0 != 0xE0 || (b1 >> 1) & 0b11 != 0b01 {
        return None;
    }
    let version = match (b1 >> 3) & 0b11 {
        0b11 => MpegVersion::Mpeg1,
        0b10 => MpegVersion::Mpeg2,
        0b00 => MpegVersion::Mpeg25,
        _ => return None,
    };
    let bitrates = if version == MpegVersion::Mpeg1 {
        &MPEG1_BITRATES
    } else {
        &MPEG2_BITRATES
    };
    let bitrate_kbps = *bitrates
        .get(usize::from(b2 >> 4))
        .filter(|&&kbps| kbps != 0)?;
    let sample_rate = MPEG1_SAMPLE_RATES.get(usize::from((b2 >> 2) & 0b11))?
        / match version {
            MpegVersion::Mpeg1 => 1,
            MpegVersion::Mpeg2 => 2,
            MpegVersion::Mpeg25 => 4,
        };
    let padding = usize::from((b2 >> 1) & 1);
    let channels = if b3 >> 6 == 0b11 { 1 } else { 2 };

    let bytes_per_kbps = if version == MpegVersion::Mpeg1 {
        144
    } else {
        72
    };
    let len = (bytes_per_kbps * bitrate_kbps * 1000 / sample_rate) as usize + padding;
    let format = Format {
        version,
        bitrate_kbps,
        sample_rate,
        channels,
    };
    Some((format, len))
}

/// Converts changes to the format of the stock clip they replace.
pub trait Convert {
    /// Converts the audio file `change` to the format of `stock`.
    fn convert(&self, change: &Path, stock: &[u8]) -> io::Result<Vec<u8>>;
    /// A short silent clip in the format of `stock`.
    fn silence(&self, stock: &[u8]) -> io::Result<Vec<u8>>;
}

/// Converts with ffmpeg, which must be on the PATH. Every clip it makes is measured again and
/// refused unless it has the stock clip's format.
pub struct Ffmpeg;

impl Convert for Ffmpeg {
    fn convert(&self, change: &Path, stock: &[u8]) -> io::Result<Vec<u8>> {
        let format = measure_stock(stock)?;
        encode([OsStr::new("-i"), change.as_os_str()], format).map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("cannot convert {}: {e}", change.display()),
            )
        })
    }

    fn silence(&self, stock: &[u8]) -> io::Result<Vec<u8>> {
        let format = measure_stock(stock)?;
        let source = format!(
            "anullsrc=r={}:cl={}",
            format.sample_rate,
            format.channel_layout()
        );
        encode(
            ["-f", "lavfi", "-i", &source, "-t", SILENCE_SECONDS],
            format,
        )
        .map_err(|e| io::Error::new(e.kind(), format!("cannot make a silent clip: {e}")))
    }
}

fn measure_stock(stock: &[u8]) -> io::Result<Format> {
    measure(stock).map_err(|e| invalid(format!("cannot measure the format of the stock clip: {e}")))
}

/// Runs ffmpeg on `input` and encodes its first audio stream as a bare MP3 in `format`: constant
/// bitrate, no ID3 tag and no Xing/Info frame, like the stock clips.
fn encode(
    input: impl IntoIterator<Item = impl AsRef<OsStr>>,
    format: Format,
) -> io::Result<Vec<u8>> {
    let output = Command::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-loglevel", "error"])
        .args(input)
        .args(["-map", "0:a:0", "-map_metadata", "-1", "-c:a", "libmp3lame"])
        .args(["-b:a", &format!("{}k", format.bitrate_kbps)])
        .args(["-ar", &format.sample_rate.to_string()])
        .args(["-ac", &format.channels.to_string()])
        .args(["-write_xing", "0", "-id3v2_version", "0", "-f", "mp3", "-"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => io::Error::new(
                e.kind(),
                "ffmpeg was not found; install it, or run the tool with `nix run`",
            ),
            _ => e,
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(io::Error::other(format!(
            "ffmpeg failed ({}): {}",
            output.status,
            stderr.trim()
        )));
    }
    let converted =
        measure(&output.stdout).map_err(|e| invalid(format!("ffmpeg made a broken MP3: {e}")))?;
    if converted != format {
        return Err(invalid(format!(
            "ffmpeg made {converted} instead of {format}"
        )));
    }
    Ok(output.stdout)
}
