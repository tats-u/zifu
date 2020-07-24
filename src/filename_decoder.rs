use ansi_term::Color::{Green, Red};
use locale_config::Locale;

pub trait IDecoder {
    fn to_string_lossless(&self, input: &Vec<u8>) -> Option<String>;
    fn to_string_lossy(&self, input: &Vec<u8>) -> String;
    fn encoding_name(&self) -> &str;
    fn color(&self) -> ansi_term::Color;
}

struct UTF8IdentityDecoder {}

struct LegacyEncodingDecoder {
    decoder: &'static encoding_rs::Encoding,
}

impl IDecoder for UTF8IdentityDecoder {
    fn to_string_lossless(&self, input: &Vec<u8>) -> Option<String> {
        return match String::from_utf8(input.to_vec()) {
            Ok(s) => Some(s),
            Err(e) => None,
        };
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

    pub fn windows_legacy_encoding() -> Box<dyn IDecoder> {
        let current_locale_name_full = Locale::user_default().to_string();
        let current_locale_name = &current_locale_name_full[0..5];
        let current_language = &current_locale_name_full[0..2];
        return Box::new(LegacyEncodingDecoder {
            decoder: match current_language {
                "ja" => encoding_rs::SHIFT_JIS,
                "zh" => match current_locale_name {
                    "zh-CN" | "zh-SG" => encoding_rs::GBK,
                    _ => encoding_rs::BIG5,
                },
                "ko" => encoding_rs::EUC_KR,
                "th" => encoding_rs::WINDOWS_874,
                "pl" | "cs" | "sk" | "hu" | "bs" | "hr" | "sr" | "ro" | "sq" => {
                    encoding_rs::WINDOWS_1250
                }
                "ru" | "bg" | "mk" => encoding_rs::WINDOWS_1251,
                // 1252 => fallback
                "el" => encoding_rs::WINDOWS_1253,
                "tr" => encoding_rs::WINDOWS_1254,
                "he" => encoding_rs::WINDOWS_1255,
                "ar" => encoding_rs::WINDOWS_1256,
                "et" | "lv" | "lt" => encoding_rs::WINDOWS_1257,
                "vi" => encoding_rs::WINDOWS_1258,
                _ => encoding_rs::WINDOWS_1252,
            },
        });
    }

    pub fn from_encoding_name(name: &str) -> Option<Box<dyn IDecoder>> {
        if let Some(decoder) = encoding_rs::Encoding::for_label(name.as_bytes()) {
            return Some(Box::new(LegacyEncodingDecoder { decoder: decoder }));
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
