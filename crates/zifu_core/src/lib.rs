use byteorder::{ReadBytesExt, WriteBytesExt};
use filename_decoder::{ASCIIDecoder, IDecoder};
use hfs_nfd::compose_from_hfs_nfd;
use zip_structs::{
    zip_central_directory::ZipCDEntry, zip_eocd::ZipEOCD, zip_error::ZipReadError,
    zip_local_file_header,
};

pub mod filename_decoder;

static ASCII_DECODER: ASCIIDecoder = ASCIIDecoder {};

/// This is for listing file names

/// This is for listing file names
#[derive(Clone, Debug)]
pub struct FileNameEntry {
    /// File name (or path)
    pub name: String,
    /// Explicitly UTF-8 encoded (general purpose flag #11)
    ///
    /// If `false`, print the encoding name whose decoder you used
    pub encoding_type: FileNameEncodingType,
}

/// Enum that represents statistics of file name encoding
///
/// UTF-8 (Regular normalization (NFC) / Irregular (HFS+ NFD-like)) / ASCII / Implicit multibyte
#[derive(Debug, Clone)]
pub enum FileNameEncodingType {
    /// genral bit #11 + NFC normalization (universal)
    ExplicitRegularUTF8,
    /// general bit #11 + non-NFC normalization (e.g. HFS+ NFD-like normalization)
    ///
    /// Used by e.g. Finder in macOS
    ExplicitIrregularUTF8,
    /// no general bit #11 + ASCII (universal)
    ImplicitASCII,
    /// no general bit #11 + non-ASCII encoding (e.g. CP437, Shift-JIS, or UTF-8)
    ImplicitNonASCII,
}

impl FileNameEncodingType {
    /// Regturns `true` if the file name is correctly decoded in almost all devices
    pub fn is_universal(&self) -> bool {
        use FileNameEncodingType::*;
        match self {
            ExplicitRegularUTF8 | ImplicitASCII => true,
            _ => false,
        }
    }
}

/// Represents diagnostic result of the file names
#[derive(Clone, Debug)]
pub struct FileNamesDiagnosis {
    /// `true` if contains implicit (general purpose bit #11 not set) non-ASCII
    /// (e.g. UTF-8, CP437, or Shift-JIS) file names
    pub has_implicit_non_ascii_names: bool,
    /// contains explicit (general purpose bit #11) irregular (e.g. HFS+ NFD) file names
    pub has_non_nfc_explicit_utf8_names: bool,
}

impl FileNamesDiagnosis {
    /// Getprimary message to explain name encoding status
    pub fn get_status_primary_message(&self) -> &'static str {
        match (self.has_implicit_non_ascii_names, self.has_non_nfc_explicit_utf8_names) {
            (false, false) => "All file names are encoded in ASCII or explicitly in UTF-8.",
            (true, false) => "Some files are encoded implicitly in a multibyte encoding.",
            (false, true) => "Some file names use irregular unicode normalization.",
            (true, true) => "Some files use irregular unicode normalization and others are encoded implicitly in a multibyte encoding.",
        }
    }
    /// Get note to explain name encoding status (if exists)
    ///
    /// Use with `.get_status_primary_message()`
    pub fn get_status_note(&self) -> &'static str {
        match (self.has_implicit_non_ascii_names, self.has_non_nfc_explicit_utf8_names) {
            (false, false) => "Almost all devices can decode its file names correctly.",
            (true, _) => "Apply this tool, or the receiver may not be able to see the correct file names.",
            (false, true) => "Apply this tool, or the receiver may not deal with the pericular file name normalization.",
        }
    }

    /// Returns `true` if the ZIP archive is universal (do not have to apply this tool)
    pub fn is_universal_archive(&self) -> bool {
        return !self.has_implicit_non_ascii_names && !self.has_non_nfc_explicit_utf8_names;
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

    /// Returns the file name encoding diagnossis.
    ///
    /// For details, see the description for `FileNamesDiagnosis`.
    pub fn diagnose_file_name_encoding(&self) -> FileNamesDiagnosis {
        FileNamesDiagnosis {
            has_implicit_non_ascii_names: self
                .cd_entries
                .iter()
                .filter(|cd| !cd.is_encoded_in_utf8())
                .any(|cd| !ASCII_DECODER.can_decode(&cd.file_name_raw)),
            has_non_nfc_explicit_utf8_names: self
                .cd_entries
                .iter()
                .filter(|cd| cd.is_encoded_in_utf8())
                .any(|cd| {
                    let original_name = String::from_utf8_lossy(&cd.file_name_raw);
                    let nfc_name = compose_from_hfs_nfd(&original_name);
                    &original_name != &nfc_name
                }),
        }
    }

    /// Test applying given decoders to the file names and returns the index of the first successful one.
    ///
    /// If nothing is successful for all names, returns `None`.
    ///
    /// # Arguments
    ///
    /// * `decoders_list` - list of decoders; the former the higher priority.
    pub fn get_filename_decoder_index(&self, decoders_list: &[&dyn IDecoder]) -> Option<usize> {
        return filename_decoder::decide_decoder(
            decoders_list,
            &(&self
                .cd_entries
                .iter()
                .flat_map(|cd| vec![&cd.file_name_raw, &cd.file_comment])
                .collect::<Vec<&Vec<u8>>>()),
        );
    }

    /// Returns a list of file names (including whether they are explicitly encoded in UTF-8).
    ///
    /// # Arguments
    ///
    /// * `legacy_decoder` - used for implicitly-encoded file names.
    pub fn get_file_names_list(&self, legacy_decoder: &dyn IDecoder) -> Vec<FileNameEntry> {
        use FileNameEncodingType::*;
        self.cd_entries
            .iter()
            .map(|cd| {
                if cd.is_encoded_in_utf8() {
                    let original_file_name = String::from_utf8_lossy(&*(cd.file_name_raw));
                    let nfc_file_name = compose_from_hfs_nfd(&original_file_name);
                    return FileNameEntry {
                        encoding_type: if &original_file_name == &nfc_file_name {
                            ExplicitRegularUTF8
                        } else {
                            ExplicitIrregularUTF8
                        },
                        name: nfc_file_name,
                    };
                }
                if let Some(ascii_file_name) = ASCII_DECODER.to_string_lossless(&cd.file_name_raw) {
                    return FileNameEntry {
                        encoding_type: ImplicitASCII,
                        name: ascii_file_name,
                    };
                }
                return FileNameEntry {
                    encoding_type: ImplicitNonASCII,
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
                let original_file_name = String::from_utf8_lossy(&cd.file_name_raw);
                let nfc_file_name = compose_from_hfs_nfd(&original_file_name);
                if original_file_name != nfc_file_name {
                    cd.set_file_name_from_slice(&nfc_file_name.as_bytes().to_vec());
                }
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
            }
            // We don't have to consider the case that the UTF-8 flag only in the local file header is set (very rare & non-RFC)
            if cd.is_encoded_in_utf8() {
                local_header.set_utf8_encoded_flag();
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
