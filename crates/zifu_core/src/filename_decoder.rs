use codepage::to_encoding;
use lazy_static::lazy_static;
use locale_name_code_page::get_codepage;
use oem_cp::code_table::DECODING_TABLE_CP_MAP;
use oem_cp::code_table_type::TableType;
use regex::Regex;

use hfs_nfd::compose_from_hfs_nfd;
use locale_config::Locale;

/// Trait (interface) of decoder
pub trait IDecoder {
    /// Converts to UTF-8 `String` only if possible completely
    ///
    /// # Arguments
    ///
    /// * `input` - sequence of bytes that may represent a string
    fn to_string_lossless(&self, input: &[u8]) -> Option<String>;
    /// Converts to UTF-8 `String` by force (filling with replacement characters)
    ///
    /// # Arguments
    ///
    /// * `input` - sequence of bytes that may represent a string
    fn to_string_lossy(&self, input: &[u8]) -> String;
    /// Returns `true` if `input` is valid sequence for encoding
    ///
    /// # Arguments
    /// * `input` - sequence of bytes that may represent a string
    fn can_decode(&self, input: &[u8]) -> bool {
        self.to_string_lossless(input).is_some()
    }
    /// Returns the name of the encoding that the decoder uses
    fn encoding_name(&self) -> &str;
}

/// UTF-8 decoder
///
/// Also normalize NFD (Apple's variant) encoded names to NFC.
pub struct UTF8NFCDecoder {}

/// ASCII decoder
///
/// Allows only <= U+7F characters
pub struct ASCIIDecoder {}

/// OEM code page decoder (other than Asian languages)
///
/// Single byte & use `oem_cp` to implement
struct OEMCPDecoder {
    decoder: &'static TableType,
    encoding_str: String,
}

/// Asian ANSI+OEM codepages decoder
///
/// Use encoding_rs (CJKV + Thai)
struct LegacyEncodingDecoder {
    /// `Encoding` object (e.g. `encoding_rs::SHIFT_JIS` for Shift-JIS)
    decoder: &'static encoding_rs::Encoding,
}

impl IDecoder for UTF8NFCDecoder {
    fn to_string_lossless(&self, input: &[u8]) -> Option<String> {
        return String::from_utf8(input.to_vec())
            .map(|s| compose_from_hfs_nfd(&s))
            .ok();
    }
    fn to_string_lossy(&self, input: &[u8]) -> String {
        return compose_from_hfs_nfd(&String::from_utf8_lossy(input));
    }
    fn can_decode(&self, input: &[u8]) -> bool {
        return std::str::from_utf8(input).is_ok();
    }
    fn encoding_name(&self) -> &str {
        return "UTF-8";
    }
}

impl IDecoder for ASCIIDecoder {
    fn to_string_lossless(&self, input: &[u8]) -> Option<String> {
        if input.iter().any(|c| !c.is_ascii()) {
            return None;
        }
        // UTF-8 is upper compatible with ASCII
        return String::from_utf8(input.to_vec()).ok();
    }
    fn to_string_lossy(&self, input: &[u8]) -> String {
        return input
            .iter()
            .map(|c| if c.is_ascii() { *c as char } else { '\u{FFFD}' })
            .collect();
    }
    fn can_decode(&self, input: &[u8]) -> bool {
        return input.iter().all(|c| c.is_ascii());
    }
    fn encoding_name(&self) -> &str {
        return "ASCII";
    }
}

impl OEMCPDecoder {
    fn from_codepage(codepage: u16) -> Option<Self> {
        return Some(Self {
            decoder: DECODING_TABLE_CP_MAP.get(&codepage)?,
            encoding_str: format!("CP{}", codepage),
        });
    }
    fn fallback() -> Self {
        return Self::from_codepage(437).unwrap();
    }
}

impl IDecoder for OEMCPDecoder {
    fn to_string_lossless(&self, input: &[u8]) -> Option<String> {
        return self.decoder.decode_string_checked(input);
    }
    fn to_string_lossy(&self, input: &[u8]) -> String {
        return self.decoder.decode_string_lossy(input);
    }
    fn encoding_name(&self) -> &str {
        return &self.encoding_str;
    }
}

impl IDecoder for LegacyEncodingDecoder {
    fn to_string_lossless(&self, input: &[u8]) -> Option<String> {
        let (result, _, met_invalid_char) = self.decoder.decode(&input);
        if met_invalid_char {
            return None;
        }
        return Some(result.into_owned());
    }
    fn to_string_lossy(&self, input: &[u8]) -> String {
        return self.decoder.decode(&input).0.into_owned();
    }
    fn encoding_name(&self) -> &str {
        return self.decoder.name();
    }
    fn can_decode(&self, input: &[u8]) -> bool {
        !self.decoder.decode(input).2
    }
}

impl dyn IDecoder {
    /// Returns UTF-8 decoder
    pub fn utf8() -> Box<dyn IDecoder> {
        return Box::new(UTF8NFCDecoder {});
    }
    /// Returns ASCII decoder
    pub fn ascii() -> Box<dyn IDecoder> {
        return Box::new(ASCIIDecoder {});
    }

    /// Returns native OEM code pages for the current locale
    ///
    /// Supported: CJKV / Thai / IBM OEM
    pub fn native_oem_encoding() -> Box<dyn IDecoder> {
        let current_locale_name_full = Locale::user_default().to_string();
        if let Some(codepage) = get_codepage(current_locale_name_full) {
            if let Some(encoding) = to_encoding(codepage.oem) {
                return Box::new(LegacyEncodingDecoder { decoder: encoding });
            }
            if let Some(decoder) = OEMCPDecoder::from_codepage(codepage.oem) {
                return Box::new(decoder);
            }
        }
        return Box::new(OEMCPDecoder::fallback());
    }

    /// Generates an instance of a decoder from encoding name (e.g. `sjis` -> Shift-JIS)
    ///
    /// # Arguments
    ///
    /// * `name` - encoding name
    pub fn from_encoding_name(name: &str) -> Option<Box<dyn IDecoder>> {
        lazy_static! {
            static ref OEM_CP_REGEX: Regex = Regex::new(r"(?i)(?:CP|OEM ?|IBM)(\d+)").unwrap();
            static ref CP437_REGEX: Regex =
                Regex::new("(?i)(OEM[-_]US|PC-8|DOS[-_ ]?Latin[-_ ]?US)").unwrap();
        }
        if let Some(decoder) = encoding_rs::Encoding::for_label(name.as_bytes()) {
            return Some(Box::new(LegacyEncodingDecoder { decoder: decoder }));
        }
        if let Some(decoder) = OEM_CP_REGEX
            .captures(name)
            .and_then(|captures| captures.get(1))
            .and_then(|match_| -> Option<u16> { match_.as_str().parse().ok() })
            .and_then(|codepage| OEMCPDecoder::from_codepage(codepage))
        {
            return Some(Box::new(decoder));
        }
        if CP437_REGEX.is_match(name) {
            return Some(Box::new(OEMCPDecoder::fallback()));
        }
        return None;
    }
}

/// Guesses encoding from an array of sequences.
/// Returns an index of the array `decoders` corresponding to the encoding that was able to decode all the `strings` without error.
/// If no `decoders` can decode all of `strings` without error, returns `None`.
///
/// # Arguments
///
/// * `decoders` - encoding candidates.  The smaller the index, the higher the priority
/// * `strings` - strings that an encoding must be able to decode all of them
pub fn decide_decoder<T>(decoders: &[&dyn IDecoder], strings: &[T]) -> Option<usize>
where
    T: AsRef<[u8]>,
{
    for i in 0..decoders.len() {
        let decoder = decoders[i];
        if strings
            .into_iter()
            .all(|subject| decoder.can_decode(subject.as_ref()))
        {
            return Some(i);
        }
    }
    return None;
}
