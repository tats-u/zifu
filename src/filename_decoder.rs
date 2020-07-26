use ansi_term::Color::{Green, Red};
use codepage_437::{FromCp437, CP437_CONTROL};
use locale_config::Locale;

pub trait IDecoder {
    fn to_string_lossless(&self, input: &Vec<u8>) -> Option<String>;
    fn to_string_lossy(&self, input: &Vec<u8>) -> String;
    fn encoding_name(&self) -> &str;
    fn color(&self) -> ansi_term::Color;
}

struct UTF8IdentityDecoder {}

struct ASCIIDecoder {}

struct CP437Decoder {}

struct LegacyEncodingDecoder {
    decoder: &'static encoding_rs::Encoding,
}

impl IDecoder for UTF8IdentityDecoder {
    fn to_string_lossless(&self, input: &Vec<u8>) -> Option<String> {
        return String::from_utf8(input.to_vec()).ok();
    }
    fn to_string_lossy(&self, input: &Vec<u8>) -> String {
        return String::from_utf8_lossy(&input).to_string();
    }
    fn encoding_name(&self) -> &str {
        return "UTF-8";
    }
    fn color(&self) -> ansi_term::Color {
        return Green;
    }
}

impl IDecoder for ASCIIDecoder {
    fn to_string_lossless(&self, input: &Vec<u8>) -> Option<String> {
        if input.iter().any(|c| !c.is_ascii()) {
            return None;
        }
        return String::from_utf8(input.to_vec()).ok();
    }
    fn to_string_lossy(&self, input: &Vec<u8>) -> String {
        return input
            .iter()
            .map(|c| if c.is_ascii() { *c as char } else { '?' })
            .collect();
    }
    fn encoding_name(&self) -> &str {
        return "ASCII";
    }
    fn color(&self) -> ansi_term::Color {
        return Green;
    }
}

impl IDecoder for CP437Decoder {
    fn to_string_lossless(&self, input: &Vec<u8>) -> Option<String> {
        return Some(String::from_cp437(input.clone(), &CP437_CONTROL));
    }
    fn to_string_lossy(&self, input: &Vec<u8>) -> String {
        return String::from_cp437(input.clone(), &CP437_CONTROL);
    }
    fn encoding_name(&self) -> &str {
        return "CP437";
    }
    fn color(&self) -> ansi_term::Color {
        return Red;
    }
}

impl IDecoder for LegacyEncodingDecoder {
    fn to_string_lossless(&self, input: &Vec<u8>) -> Option<String> {
        let (result, _, met_invalid_char) = self.decoder.decode(&input);
        if met_invalid_char {
            return None;
        }
        return Some(result.into_owned());
    }
    fn to_string_lossy(&self, input: &Vec<u8>) -> String {
        return self.decoder.decode(&input).0.into_owned();
    }
    fn encoding_name(&self) -> &str {
        return self.decoder.name();
    }
    fn color(&self) -> ansi_term::Color {
        return Red;
    }
}

impl dyn IDecoder {
    pub fn utf8() -> Box<dyn IDecoder> {
        return Box::new(UTF8IdentityDecoder {});
    }
    pub fn ascii() -> Box<dyn IDecoder> {
        return Box::new(ASCIIDecoder {});
    }

    pub fn native_oem_encoding() -> Box<dyn IDecoder> {
        let current_locale_name_full = Locale::user_default().to_string();
        let current_locale_name = &current_locale_name_full[0..5];
        let current_language = &current_locale_name_full[0..2];
        let encoding = match current_language {
            "ja" => Some(encoding_rs::SHIFT_JIS),
            "zh" => match current_locale_name {
                "zh-CN" | "zh-SG" => Some(encoding_rs::GBK),
                _ => Some(encoding_rs::BIG5),
            },
            "ko" => Some(encoding_rs::EUC_KR),
            "th" => Some(encoding_rs::WINDOWS_874),
            // "pl" | "cs" | "sk" | "hu" | "bs" | "hr" | "sr" | "ro" | "sq" => {
            //     Some(encoding_rs::WINDOWS_1250)
            // },
            // "ru" | "bg" | "mk" => encoding_rs::WINDOWS_1251,
            // 1252 => fallback
            // "el" => encoding_rs::WINDOWS_1253,
            // "tr" => encoding_rs::WINDOWS_1254,
            // "he" => encoding_rs::WINDOWS_1255,
            // "ar" => encoding_rs::WINDOWS_1256,
            // "et" | "lv" | "lt" => encoding_rs::WINDOWS_1257,
            "vi" => Some(encoding_rs::WINDOWS_1258),
            _ => None,
        };
        if encoding.is_some() {
            return Box::new(LegacyEncodingDecoder {
                decoder: encoding.unwrap(),
            });
        }
        return Box::new(CP437Decoder {});
    }

    pub fn from_encoding_name(name: &str) -> Option<Box<dyn IDecoder>> {
        if let Some(decoder) = encoding_rs::Encoding::for_label(name.as_bytes()) {
            return Some(Box::new(LegacyEncodingDecoder { decoder: decoder }));
        }
        if regex::Regex::new("(?i)((CP|OEM ?)437|OEM[-_]US|PC-8|DOS[-_ ]?Latin[-_ ]?US)")
            .map(|r| r.is_match(name))
            .unwrap_or(false)
        {
            return Some(Box::new(CP437Decoder {}));
        }
        return None;
    }
}

pub fn decide_decoeder(
    decoders: &Vec<&Box<dyn IDecoder>>,
    strings: &Vec<&Vec<u8>>,
) -> Option<usize> {
    for i in 0..decoders.len() {
        let decoder = &decoders[i];
        if strings
            .iter()
            .all(|subject| decoder.to_string_lossless(subject) != None)
        {
            return Some(i);
        }
    }
    return None;
}
