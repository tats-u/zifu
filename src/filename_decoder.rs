pub trait IDecoder {
    fn to_string_lossless(&self, input: &Vec<u8>) -> Option<String>;
    fn to_string_lossy(&self, input: &Vec<u8>) -> String;
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
}

pub struct FileNameDecoder {
    pub language: String,
    decoder: Box<dyn IDecoder>,
}

impl FileNameDecoder {
    pub fn init(prefer_lang: Option<String>, force_utf8: bool) -> Self {
        if force_utf8 {
            return Self {
                language: prefer_lang.unwrap_or("ja".to_string()),
                decoder: Box::new(UTF8IdentityDecoder {}),
            };
        }
        return Self {
            language: prefer_lang.unwrap_or("ja".to_string()),
            decoder: Box::new(LegacyEncodingDecoder {
                decoder: encoding_rs::SHIFT_JIS,
            }),
        };
    }
    pub fn to_string_lossless(&self, input: &Vec<u8>) -> Option<String> {
        return self.decoder.to_string_lossless(input);
    }
    pub fn to_string_lossy(&self, input: &Vec<u8>) -> String {
        return self.decoder.to_string_lossy(input);
    }
}
