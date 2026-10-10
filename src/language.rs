//! The voice-guidance languages Sony publishes a stock voice pack for.

use clap::ValueEnum;

/// A voice-guidance language, numbered with the code the headphones and Sony's update server use.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Language {
    English = 0x01,
    French = 0x02,
    German = 0x03,
    Spanish = 0x04,
    Italian = 0x05,
    Portuguese = 0x06,
    Dutch = 0x07,
    Swedish = 0x08,
    Finnish = 0x09,
    Turkish = 0x10,
}

impl Language {
    /// The language code, which also starts the unpacked image of a voice pack in this language.
    pub fn code(self) -> u32 {
        self as u32
    }

    /// The language with `code`, if it is one of these.
    pub fn from_code(code: u32) -> Option<Self> {
        Self::value_variants()
            .iter()
            .copied()
            .find(|language| language.code() == code)
    }

    /// The language pack's directory on the update server, such as `VGIDLPB0401` for English. It
    /// ends in the language code in hex, so Turkish (0x10) is `VGIDLPB0410`.
    pub fn pack_id(self) -> String {
        format!("VGIDLPB04{:02X}", self.code())
    }

    /// The name of the language, as `--lang` and `--install-lang` take it.
    pub fn name(self) -> String {
        self.to_possible_value()
            .expect("no language is skipped")
            .get_name()
            .to_string()
    }
}
