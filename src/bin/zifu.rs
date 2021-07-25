use ansi_term::Color::{Green, Red, Yellow};
use anyhow::anyhow;
use byteorder::{ReadBytesExt, WriteBytesExt};
use clap::{crate_authors, crate_description, crate_version, App, Arg, ArgMatches};
use filename_decoder::IDecoder;
use std::borrow::Cow;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use zifu::filename_decoder;
use zip_central_directory::ZipCDEntry;
use zip_eocd::ZipEOCD;
use zip_structs::zip_central_directory;
use zip_structs::zip_eocd;
use zip_structs::zip_local_file_header;

#[derive(thiserror::Error, Debug)]
enum InvalidArgument {
    #[error("no argument <{arg_name}> was passed")]
    NoArgument { arg_name: String },
    #[error("unknown encoding name: {encoding_name}")]
    InvalidEncodingName { encoding_name: String },
    #[error("you cannot specify the same file for input and output files.")]
    SameInputOutput,
}

/// Global behavior options for this program
#[derive(Debug, Clone, Copy, Default)]
struct GlobalFlags {
    verbose: bool,
    ask_user: bool,
}

/// Enum that
#[derive(Debug)]
enum ZipFileEncodingType {
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

/// Returns reset given ANSI style if non-tty
fn prepare_for_non_tty(style: ansi_term::Style) -> ansi_term::Style {
    if atty::is(atty::Stream::Stdout) {
        style
    } else {
        ansi_term::Style::default()
    }
}

impl ZipFileEncodingType {
    /// Get primary message to explain name encoding status
    fn get_status_primary_message(&self) -> Cow<'static, str> {
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
    fn get_status_note(&self) -> Option<&'static str> {
        use ZipFileEncodingType::*;
        match self {
            ExplicitUTF8AndASCII { .. } | AllASCII => {
                Some("They can be extracted correctly in all environments without garbling.")
            }
            _ => None,
        }
    }
    /// Get color for `.get_status_primary_message()`
    fn get_status_color(&self) -> ansi_term::Colour {
        use ZipFileEncodingType::*;
        match self {
            AllExplicitUTF8 => Green,
            ExplicitUTF8AndASCII { .. } | AllASCII => Yellow,
            _ => Red,
        }
    }
    /// Prints messages (`.get_status_primary_message()` & `.get_statius_note()`)
    pub fn print_status_message(&self) {
        if let Some(note) = self.get_status_note() {
            println!(
                "{}  {}",
                prepare_for_non_tty(self.get_status_color().bold())
                    .paint(self.get_status_primary_message()),
                prepare_for_non_tty(Green.bold()).paint(note)
            );
        } else {
            println!(
                "{}",
                prepare_for_non_tty(self.get_status_color().bold())
                    .paint((self.get_status_primary_message()).as_ref())
            );
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
}

/// Returns `Ok(ZipFileEncodingType)` , `Err(...)` (anyhow's) when an error occurs in validation
///
/// # Arguments
///
/// * `eocd` - EOCD
/// * `cd_entries` - Central directories
fn check_archive(eocd: &ZipEOCD, cd_entries: &[ZipCDEntry]) -> anyhow::Result<ZipFileEncodingType> {
    use ZipFileEncodingType::*;

    let utf8_entries_count = cd_entries
        .iter()
        .filter(|&cd| cd.is_encoded_in_utf8())
        .count();
    if utf8_entries_count == eocd.n_cd_entries as usize {
        return Ok(AllExplicitUTF8);
    }
    let ascii_decoder = filename_decoder::IDecoder::ascii();
    if filename_decoder::decide_decoeder(
        &vec![&ascii_decoder],
        &cd_entries
            .iter()
            .flat_map(|cd| vec![&cd.file_name_raw, &cd.file_comment])
            .collect(),
    )
    .is_some()
    {
        return Ok(if utf8_entries_count > 0 {
            ExplicitUTF8AndASCII {
                n_ascii: utf8_entries_count,
                n_utf8: eocd.n_cd_entries as usize - utf8_entries_count,
            }
        } else {
            AllASCII
        });
    }
    if utf8_entries_count > 0 {
        return Ok(ExplicitUTF8AndLegacy {
            n_legacy: eocd.n_cd_entries as usize - utf8_entries_count,
            n_utf8: utf8_entries_count,
        });
    }
    return Ok(AllLegacy);
}

/// Decodes and prints file names in central directories to stdout
///
/// # Arguments
///
/// * `cd_entries` - Central directories (contains file names)
/// * `utf8_decoder` - UTF-8 decoder (used when explicitly encoded in UTF-8)
/// * `legacy_decoder` - Legacy charset decoder (used otherwise)
fn list_names_in_archive(
    cd_entries: &[ZipCDEntry],
    utf8_decoder: &dyn IDecoder,
    legacy_decoder: &dyn IDecoder,
) {
    for cd in cd_entries {
        if cd.is_encoded_in_utf8() {
            println!(
                "{}:{}:{}",
                Green.bold().paint("EXPLICIT"),
                Green.bold().paint("UTF-8"),
                utf8_decoder.to_string_lossy(&cd.file_name_raw)
            );
        } else {
            println!(
                "{}:{}:{}",
                Red.bold().paint("GUESSED"),
                legacy_decoder
                    .color()
                    .bold()
                    .paint(legacy_decoder.encoding_name()),
                legacy_decoder.to_string_lossy(&cd.file_name_raw)
            );
        }
    }
}

/// Generates ZIP archive
///
/// Fixes position and size entries in EOCD at the same time
///
/// # Arguments
///
/// * `zip_file` - File object for input ZIP file
/// * `eocd` - EOCD struct
/// * `cd_entries` - Vector of central directories
/// * `decoder` - ASCII-compatible character set for converting file names encoded in it to UTF-8
/// * `output_sip_file` - File object for output ZIP file
// TODO: Split processes into fixing encoding & outputting ZIP file
fn output_zip_archive<R: ReadBytesExt + std::io::Seek, W: WriteBytesExt>(
    zip_file: &mut R,
    eocd: &mut ZipEOCD,
    cd_entries: &mut Vec<ZipCDEntry>,
    decoder: &dyn IDecoder,
    output_zip_file: &mut W,
) -> anyhow::Result<()> {
    // Writer can't get the current position, so we must record it by ourselves.
    let mut pos: u64 = 0;
    // Local header (including contents)
    for cd in cd_entries.iter_mut() {
        let mut local_header =
            zip_local_file_header::ZipLocalFileHeader::from_central_directory(zip_file, cd)?;
        if !cd.is_encoded_in_utf8() {
            let file_name_u8 = decoder.to_string_lossy(&cd.file_name_raw);
            let file_name_u8bytes = file_name_u8.as_bytes().to_vec();

            cd.set_file_name_from_slice(&file_name_u8bytes);
            local_header.set_file_name_from_slice(&file_name_u8bytes);
            let file_comment_u8 = decoder.to_string_lossy(&cd.file_comment);
            let file_comment_u8bytes = file_comment_u8.as_bytes().to_vec();
            cd.set_file_coment_from_slice(&file_comment_u8bytes);
            cd.set_utf8_encoded_flag();
            local_header.set_utf8_encoded_flag();
        }
        cd.local_header_position = pos as u32;
        pos += local_header.write(output_zip_file)?;
    }
    // Central directory
    eocd.cd_starting_position = pos as u32;
    let mut cd_new_size: u64 = 0;
    for cd in cd_entries.iter_mut() {
        cd_new_size += cd.write(output_zip_file)?;
    }
    // EOCD
    eocd.cd_size = cd_new_size as u32;
    eocd.write(output_zip_file)?;
    return Ok(());
}

fn process_answer_default_yes(ans: &str) -> bool {
    return match ans.chars().next() {
        Some('n') | Some('N') => false,
        None | Some(_) => true,
    };
}

/// Returns `Ok(false)` if a line starting with `'n'` (or `'N'`) is input from stdin, otherwise `Ok(true)`.
///
/// Returns `Err(std::io::Error)` if I/O fails.
fn ask_default_yes() -> Result<bool, std::io::Error> {
    let ask_result = (|| {
        let mut ret = String::new();
        match std::io::stdin().read_line(&mut ret) {
            Ok(_) => return Ok(ret),
            Err(e) => return Err(e),
        }
    })()?;
    return Ok(process_answer_default_yes(&ask_result));
}

fn get_arg_parser() -> App<'static> {
    return App::new("ZIP File Names to UTF-8 (ZIFU)")
    .author(crate_authors!())
    .version(crate_version!())
    .about(crate_description!())
    .arg(
        Arg::new("input")
            .about("Path to the ZIP file where you want to change the encoding of the file name to UTF-8")
            .required(true)
        )
    .arg(
        Arg::new("output")
            .about("Path to output")
    )
    .arg(
        Arg::new("check")
            .long("check")
            .short('c')
            .about("Finds out if its file names are encoded in UTF-8.")
    )
    .arg(
        Arg::new("list")
            .short('l')
            .long("list")
            .about("Displays the list of file names in the ZIP archive.")
    )
    .arg(
        Arg::new("silent")
        .short('s')
        .long("slient")
        .about("Don't show any messages. (implies -y)")
    )
    .arg(
        Arg::new("quiet")
        .short('q')
        .long("quiet")
        .about("Don't show any messages. (implies -y)")
    )
    .arg(
        Arg::new("encoding")
        .long("encoding")
        .short('e')
        .value_name("ENCODING")
        .about("Specifies the encoding of file names in the ZIP archive.")
    )
    .arg(
        Arg::new("utf-8")
            .long("utf8")
            .short('u')
            .about("Treats the encoding of the ZIP archive as UTF-8 first. (Default: try legacy encoding first)")
    )
    .arg(
        Arg::new("yes")
        .long("yes")
        .short('y')
        .about("Don't confirm")
    );
}

fn matches_to_global_flags(matches: &ArgMatches) -> GlobalFlags {
    let verbose = !matches.is_present("silent") && !matches.is_present("quiet");
    let ask_user = verbose && !matches.is_present("yes");
    return GlobalFlags { verbose, ask_user };
}

fn main() -> anyhow::Result<()> {
    let app = get_arg_parser();

    let matches = app.get_matches();
    let global_flags = matches_to_global_flags(&matches);
    let mut zip_file = match matches.value_of("input") {
        None => {
            return Err(InvalidArgument::NoArgument {
                arg_name: "input".to_string(),
            }
            .into());
        }
        Some(a) => BufReader::new(File::open(a)?),
    };

    let mut eocd = ZipEOCD::from_reader(&mut zip_file)?;
    eocd.check_unsupported_zip_type()?;

    let mut cd_entries = ZipCDEntry::all_from_eocd(&mut zip_file, &eocd)?;

    if matches.is_present("check") {
        let archive_names_type = check_archive(&eocd, &cd_entries)?;
        archive_names_type.print_status_message();
        std::process::exit(if archive_names_type.is_universal_archive() {
            0
        } else {
            2
        });
    }

    let legacy_decoder = if let Some(encoding_name) = matches.value_of("encoding") {
        filename_decoder::IDecoder::from_encoding_name(encoding_name).ok_or(
            InvalidArgument::InvalidEncodingName {
                encoding_name: encoding_name.to_string(),
            },
        )?
    } else {
        filename_decoder::IDecoder::native_oem_encoding()
    };
    let utf8_decoder = filename_decoder::IDecoder::utf8();
    let ascii_decoder = filename_decoder::IDecoder::ascii();
    let decoders_list = if matches.is_present("utf-8") {
        vec![&ascii_decoder, &utf8_decoder, &legacy_decoder]
    } else {
        vec![&ascii_decoder, &legacy_decoder, &utf8_decoder]
    };
    // Detect encoding by trying decoding all of file names and comments
    let best_fit_decoder_index_ = filename_decoder::decide_decoeder(
        &decoders_list,
        &cd_entries
            .iter()
            .flat_map(|cd| vec![&cd.file_name_raw, &cd.file_comment])
            .collect(),
    );
    best_fit_decoder_index_.ok_or(anyhow!(
        "file names & comments are not encoded in UTF-8 or {}.  Try with -e <another encoding> option.",
        legacy_decoder.encoding_name()
    ))?;
    let guessed_encoder = decoders_list[best_fit_decoder_index_.unwrap()];

    if matches.is_present("list") {
        list_names_in_archive(&cd_entries, &*utf8_decoder, &**guessed_encoder);
        return Ok(());
    }
    if global_flags.verbose || global_flags.ask_user {
        list_names_in_archive(&cd_entries, &*utf8_decoder, &**guessed_encoder);
        if global_flags.ask_user {
            eprint!("Are these file names correct? [Y/n]: ");
            if !(ask_default_yes()?) {
                std::process::exit(1);
            }
        }
    }
    let output_zip_file_str = matches
        .value_of("output")
        .ok_or(InvalidArgument::NoArgument {
            arg_name: "output".to_string(),
        })?;
    if matches
        .value_of("input")
        .and_then(|input| Some(input == output_zip_file_str))
        .unwrap_or(false)
    {
        return Err(InvalidArgument::SameInputOutput.into());
    }

    let mut output_zip_file = BufWriter::new(File::create(output_zip_file_str)?);
    output_zip_archive(
        &mut zip_file,
        &mut eocd,
        &mut cd_entries,
        &**guessed_encoder,
        &mut output_zip_file,
    )?;
    return Ok(());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_args_parse_test() {
        let app = get_arg_parser();
        let matches = app.get_matches_from(vec!["zifu", "before.zip", "after.zip"]);
        let global_flags = matches_to_global_flags(&matches);

        assert_eq!(global_flags.ask_user, true);
        assert_eq!(global_flags.verbose, true);

        assert_eq!(matches.value_of("input"), Some("before.zip"));
        assert_eq!(matches.value_of("output"), Some("after.zip"));
    }

    #[test]
    fn extended_args_parse_test1() {
        let app = get_arg_parser();
        let matches =
            app.get_matches_from(vec!["zifu", "before.zip", "after.zip", "-q", "-u", "-l"]);
        let global_flags = matches_to_global_flags(&matches);

        assert_eq!(global_flags.ask_user, false);
        assert_eq!(global_flags.verbose, false);

        assert_eq!(matches.value_of("input"), Some("before.zip"));
        assert_eq!(matches.value_of("output"), Some("after.zip"));
        assert_eq!(matches.value_of("encoding"), None);
        assert_eq!(matches.is_present("utf-8"), true);
        assert_eq!(matches.is_present("check"), false);
        assert_eq!(matches.is_present("list"), true);
    }

    #[test]
    fn extended_args_parse_test2() {
        let app = get_arg_parser();
        let matches = app.get_matches_from(vec![
            "zifu",
            "before.zip",
            "after.zip",
            "-s",
            "-e",
            "sjis",
            "-c",
        ]);
        let global_flags = matches_to_global_flags(&matches);

        assert_eq!(global_flags.ask_user, false);
        assert_eq!(global_flags.verbose, false);

        assert_eq!(matches.value_of("input"), Some("before.zip"));
        assert_eq!(matches.value_of("output"), Some("after.zip"));
        assert_eq!(matches.value_of("encoding"), Some("sjis"));
        assert_eq!(matches.is_present("utf-8"), false);
        assert_eq!(matches.is_present("check"), true);
        assert_eq!(matches.is_present("list"), false);
    }

    #[test]
    fn extended_args_parse_test3() {
        let app = get_arg_parser();
        let matches = app.get_matches_from(vec![
            "zifu",
            "before.zip",
            "after.zip",
            "-y",
            "--encoding",
            "cp437",
        ]);
        let global_flags = matches_to_global_flags(&matches);

        assert_eq!(global_flags.ask_user, false);
        assert_eq!(global_flags.verbose, true);

        assert_eq!(matches.value_of("input"), Some("before.zip"));
        assert_eq!(matches.value_of("output"), Some("after.zip"));
        assert_eq!(matches.value_of("encoding"), Some("cp437"));
        assert_eq!(matches.is_present("utf-8"), false);
        assert_eq!(matches.is_present("check"), false);
        assert_eq!(matches.is_present("list"), false);
    }
}
