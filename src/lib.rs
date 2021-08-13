use std::borrow::Cow;

use byteorder::{ReadBytesExt, WriteBytesExt};
use filename_decoder::IDecoder;
use zip_structs::{
    zip_central_directory::ZipCDEntry, zip_eocd::ZipEOCD, zip_error::ZipReadError,
    zip_local_file_header,
};

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

/// This is for listing file names
#[derive(Clone, Debug)]
pub struct FileNameEntry {
    /// File name (or path)
    pub name: String,
    /// Explicitly UTF-8 encoded (general purpose flag #11)
    ///
    /// If `false`, print the encoding name whose decoder you used
    pub is_encoding_explicit: bool,
}

/// Enum that represents statistics of file name encoding
///
/// UTF-8 / ASCII / Legacy (Non-UTF-8 multibytes)
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

/// This struct is for providing the internal processing API used in the `zifu` CLI.
///
/// This helps you to create e.g. GUI version
///
/// This is initialized by `::new()` method.
pub struct InputZIPArchive<F: ReadBytesExt + std::io::Seek> {
    /// File handler for input ZIP file
    file_handler: F,
    /// End of central directory for the ZIP file represented by `file_handler`
    eocd: ZipEOCD,
    /// Central directories for the ZIP file represented by `file_handler`
    cd_entries: Vec<ZipCDEntry>,
}

impl<F> InputZIPArchive<F>
where
    F: ReadBytesExt + std::io::Seek,
{
    /// Returns an initialized instance.
    ///
    /// # Arguments
    ///
    /// * `handler` - File handler representing the input ZIP file (`Bufreader<File>` recommended)
    pub fn new(mut handler: F) -> anyhow::Result<Self> {
        let eocd = ZipEOCD::from_reader(&mut handler)?;
        let cd_entries = ZipCDEntry::all_from_eocd(&mut handler, &eocd)?;

        return Ok(Self {
            file_handler: handler,
            eocd,
            cd_entries,
        });
    }

    /// Returns the file name encoding statistics.
    ///
    /// For details, see the description for `ZipFileEncodingType`.
    pub fn check_file_name_encoding(&self) -> ZipFileEncodingType {
        use ZipFileEncodingType::*;

        let utf8_entries_count = self
            .cd_entries
            .iter()
            .filter(|&cd| cd.is_encoded_in_utf8())
            .count();
        if utf8_entries_count == self.eocd.n_cd_entries as usize {
            return AllExplicitUTF8;
        }
        let ascii_decoder = <dyn filename_decoder::IDecoder>::ascii();
        if filename_decoder::decide_decoeder(
            &vec![&*ascii_decoder],
            &*(&self
                .cd_entries
                .iter()
                .flat_map(|cd| [Cow::from(&cd.file_name_raw), Cow::from(&cd.file_comment)])
                .collect::<Vec<Cow<[u8]>>>()),
        )
        .is_some()
        {
            return if utf8_entries_count > 0 {
                ExplicitUTF8AndASCII {
                    n_ascii: utf8_entries_count,
                    n_utf8: self.eocd.n_cd_entries as usize - utf8_entries_count,
                }
            } else {
                AllASCII
            };
        }
        if utf8_entries_count > 0 {
            return ExplicitUTF8AndLegacy {
                n_legacy: self.eocd.n_cd_entries as usize - utf8_entries_count,
                n_utf8: utf8_entries_count,
            };
        }
        return AllLegacy;
    }

    /// Test applying given decoders to the file names and returns the index of the first successful one.
    ///
    /// If nothing is successful for all names, returns `None`.
    ///
    /// # Arguments
    ///
    /// * `decoders_list` - list of decoders; the former the higher priority.
    pub fn get_filename_decoder_index(&self, decoders_list: &[&dyn IDecoder]) -> Option<usize> {
        return filename_decoder::decide_decoeder(
            decoders_list,
            &*(&self
                .cd_entries
                .iter()
                .flat_map(|cd| [Cow::from(&cd.file_name_raw), Cow::from(&cd.file_comment)])
                .collect::<Vec<Cow<[u8]>>>()),
        );
    }

    /// Returns a list of file names (including whether they are explicitly encoded in UTF-8).
    ///
    /// # Arguments
    ///
    /// * `legacy_decoder` - used for implicitly-encoded file names.
    pub fn get_file_names_list(&self, legacy_decoder: &dyn IDecoder) -> Vec<FileNameEntry> {
        self.cd_entries
            .iter()
            .map(|cd| {
                if cd.is_encoded_in_utf8() {
                    return FileNameEntry {
                        is_encoding_explicit: true,
                        name: String::from_utf8_lossy(&*(cd.file_name_raw)).to_string(),
                    };
                }
                return FileNameEntry {
                    is_encoding_explicit: false,
                    name: legacy_decoder.to_string_lossy(&cd.file_name_raw),
                };
            })
            .collect()
    }

    /// Changes encoding of file names in central directories in ZIP archive
    ///
    /// This affects only on `.cd_entries`; The contents of the original ZIP file will not be overwritten.
    ///
    /// # Arguments
    ///
    /// * `legacy_decoder`: decoder for file names with implicit encoding
    pub fn convert_central_directory_file_names(&mut self, legacy_decoder: &dyn IDecoder) {
        self.cd_entries.iter_mut().for_each(|cd| {
            if cd.is_encoded_in_utf8() {
                return;
            }
            cd.set_file_name_from_slice(
                &legacy_decoder
                    .to_string_lossy(&cd.file_name_raw)
                    .as_bytes()
                    .to_vec(),
            );
            cd.set_file_coment_from_slice(
                &legacy_decoder
                    .to_string_lossy(&cd.file_comment)
                    .as_bytes()
                    .to_vec(),
            );
            cd.set_utf8_encoded_flag();
        });
    }

    /// Outputs the ZIP archive to the given handler.
    ///
    /// File names in local file headers will be ignored. That in central directories are used instead.
    ///
    /// # Arguments
    ///
    /// * `dest_handler` - The file handler representing for the output file.
    pub fn output_archive_with_central_directory_file_names<G: WriteBytesExt>(
        &mut self,
        dest_handler: &mut G,
    ) -> anyhow::Result<()> {
        // Writer can't get the current position, so we must record it by ourselves.
        let mut pos: u64 = 0;
        // Local header (including contents)
        for cd in self.cd_entries.iter_mut() {
            let mut local_header =
                zip_local_file_header::ZipLocalFileHeader::from_central_directory(
                    &mut self.file_handler,
                    cd,
                )?;
            if local_header.file_name_length != cd.file_name_length {
                local_header.set_file_name_from_slice(&cd.file_name_raw);
                if cd.is_encoded_in_utf8() {
                    local_header.set_utf8_encoded_flag();
                }
            }
            cd.local_header_position = pos as u32;
            pos += local_header.write(dest_handler)?;
        }
        // Central directory
        self.eocd.cd_starting_position = pos as u32;
        let mut cd_new_size: u64 = 0;
        for cd in self.cd_entries.iter_mut() {
            cd_new_size += cd.write(dest_handler)?;
        }
        // EOCD
        self.eocd.cd_size = cd_new_size as u32;
        self.eocd.write(dest_handler)?;
        return Ok(());
    }

    /// Returns `Err(ZipReadError)` if the archive has unsupported features (e.g. central directory encryption)
    pub fn check_unsupported_zip_type(&self) -> Result<(), ZipReadError> {
        return self.eocd.check_unsupported_zip_type();
    }
}
