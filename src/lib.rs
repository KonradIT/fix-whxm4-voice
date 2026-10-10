pub mod audio;
pub mod build;
pub mod changes;
pub mod download;
pub mod generate;
pub mod keys;
pub mod manifest;
pub mod pack;
pub mod size;
pub mod slots;
pub mod source;
pub mod workflow;
pub mod workspace;

/// An error for input that is not in the format Sony uses.
fn invalid(message: impl Into<String>) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, message.into())
}
