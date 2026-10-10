//! The seams between the protocols and Bluetooth, so that both can be faked in tests.

use std::io;
use std::time::Duration;

/// How long to wait for the headphones to say anything, unless a command needs longer.
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(5);

/// A byte stream to the headphones, such as one RFCOMM channel.
pub trait Transport {
    /// Writes all of `bytes`.
    fn send(&mut self, bytes: &[u8]) -> io::Result<()>;

    /// Returns the next bytes that arrive, at least one. Fails with [`io::ErrorKind::TimedOut`] when
    /// nothing arrives within `timeout`.
    fn receive(&mut self, timeout: Duration) -> io::Result<Vec<u8>>;
}

/// A protocol channel to the headphones that sends one command and awaits the reply to it.
pub trait Link<M> {
    /// Sends `command` and returns the reply to it. Messages that don't answer it are skipped.
    fn request(&mut self, command: &M) -> io::Result<M>;
}

/// Bytes as space-separated hex, the way the protocol notes write them.
pub fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}
