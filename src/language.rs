//! The voice-guidance languages Sony publishes a stock voice pack for.

use clap::ValueEnum;
use clap::builder::{PossibleValuesParser, TypedValueParser};

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
    Russian = 0x0A,
    Japanese = 0x0B,
    BrazilianPortuguese = 0x0D,
    Korean = 0x0F,
    Turkish = 0x10,
    Chinese = 0xF0,
}

impl Language {
    /// Whether `build` can lay a voice pack out as this language. The Japanese and Chinese stock
    /// packs count their slot offsets from 0x280000 and 0x180000, not from the 0x380000 that `build`
    /// writes, so a pack built as either would point the headphones at the wrong place.
    pub fn installable(self) -> bool {
        !matches!(self, Language::Japanese | Language::Chinese)
    }

    /// Parses an install language, offering only the [`installable`](Self::installable) ones.
    pub fn install_parser() -> impl TypedValueParser<Value = Self> {
        let installable = Self::value_variants()
            .iter()
            .filter(|language| language.installable())
            .filter_map(ValueEnum::to_possible_value);
        PossibleValuesParser::new(installable)
            .map(|name| Self::from_str(&name, false).expect("only possible values get here"))
    }

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
