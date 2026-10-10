pub mod audio;
pub mod bluetooth;
pub mod build;
pub mod changes;
pub mod device_info;
pub mod download;
pub mod generate;
pub mod keys;
pub mod language;
pub mod link;
pub mod manifest;
pub mod mdr;
pub mod pack;
pub mod race;
pub mod size;
pub mod slots;
pub mod source;
pub mod workflow;
pub mod workspace;

/// An error for input that is not in the format Sony uses.
fn invalid(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into())
}
