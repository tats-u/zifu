use std::borrow::Cow;

pub mod filename_decoder;

/// How much this tool is required
pub enum ZIFURequirement {
    /// All UTF-8 and this tool is not needed
    NotRequired,
    /// UTF-8 & ASCII (universal) and you might want to apply this tool
    MaybeRequired,
    /// Not universal and you must apply this tool
    Required,
}

/// Enum that represents
#[derive(Debug, Clone)]
pub enum ZipFileEncodingType {
    /// All entry names are explicitly encoded in UTF-8
    AllExplicitUTF8,
    /// All entry names are explicitly encoded in UTF-8 or implicitly ASCII
    ExplicitUTF8AndASCII { n_utf8: usize, n_ascii: usize },
    /// All entry names are implicitly encoded in UTF-8
    AllASCII,
    /// All entry names are explicitly encoded in UTF-8 or implicitly Non-ASCII
    ExplicitUTF8AndLegacy { n_utf8: usize, n_legacy: usize },
    /// All entry names are implicitly encoded in Non-ASCII
    AllLegacy,
}

impl ZipFileEncodingType {
    /// Get primary message to explain name encoding status
    pub fn get_status_primary_message(&self) -> Cow<'static, str> {
        use ZipFileEncodingType::*;
        match self {
            AllExplicitUTF8 => Cow::from("All file names are explicitly encoded in UTF-8."),
            ExplicitUTF8AndASCII{n_ascii,n_utf8} => Cow::from(format!(
                "{} file names are explicitly encoded in UTF-8, and {} file names are implicitly ASCII.",
                n_utf8,
                n_ascii
            )),
            AllASCII => Cow::from("All file names are implicitly encoded in ASCII."),
            ExplicitUTF8AndLegacy{n_utf8,n_legacy} => Cow::from(format!(
                "Some file names are not explicitly encoded in UTF-8. ({} / {})",
                n_legacy,
                n_utf8 + n_legacy,
            )),
            AllLegacy => Cow::from("All file names are not explicitly encoded in UTF-8.")
        }
    }
    /// Get note to explain name encoding status (if exists)
    ///
    /// Use with `.get_status_primary_message()`
    pub fn get_status_note(&self) -> Option<&'static str> {
        use ZipFileEncodingType::*;
        match self {
            ExplicitUTF8AndASCII { .. } | AllASCII => {
                Some("They can be extracted correctly in all environments without garbling.")
            }
            _ => None,
        }
    }

    /// Returns `true` if the ZIP archive is universal (do not have to apply this tool)
    pub fn is_universal_archive(&self) -> bool {
        use ZipFileEncodingType::*;
        return match self {
            AllExplicitUTF8 | AllASCII | ExplicitUTF8AndASCII { .. } => true,
            _ => false,
        };
    }

    pub fn is_zifu_required(&self) -> ZIFURequirement {
        use ZIFURequirement::*;
        use ZipFileEncodingType::*;
        match self {
            AllExplicitUTF8 => NotRequired,
            ExplicitUTF8AndASCII { .. } | AllASCII => MaybeRequired,
            _ => Required,
        }
    }
}
