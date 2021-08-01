use ansi_term::Color::{Green, Red};
use anyhow::anyhow;
use byteorder::{ReadBytesExt, WriteBytesExt};
use clap::{crate_authors, crate_description, crate_version, AppSettings, Clap};
use filename_decoder::IDecoder;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use zifu::ZipFileEncodingType;
use zifu::{filename_decoder, ZIFURequirement};
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
pub struct BehaviorFlags {
    pub verbose: bool,
    pub ask_user: bool,
}

/// Returns reset given ANSI style if non-tty
fn prepare_for_non_tty(style: ansi_term::Style) -> ansi_term::Style {
    if atty::is(atty::Stream::Stdout) {
        style
    } else {
        ansi_term::Style::default()
    }
}

fn zifu_requirement_to_color(requirement: &ZIFURequirement) -> ansi_term::Colour {
    use ansi_term::Colour::*;
    use ZIFURequirement::*;
    return match requirement {
        NotRequired => Green,
        MaybeRequired => Yellow,
        Required => Red,
    };
}

/// Prints messages (`.get_status_primary_message()` & `.get_statius_note()`)
pub fn print_status_message(encoding_type: &ZipFileEncodingType) {
    if let Some(note) = encoding_type.get_status_note() {
        println!(
            "{}  {}",
            prepare_for_non_tty(
                zifu_requirement_to_color(&encoding_type.is_zifu_required()).bold()
            )
            .paint(encoding_type.get_status_primary_message()),
            prepare_for_non_tty(Green.bold()).paint(note)
        );
    } else {
        println!(
            "{}",
            prepare_for_non_tty(
                zifu_requirement_to_color(&encoding_type.is_zifu_required()).bold()
            )
            .paint((encoding_type.get_status_primary_message()).as_ref())
        );
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
    let ascii_decoder = <dyn filename_decoder::IDecoder>::ascii();
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

#[derive(Clap)]
#[clap(name = "ZIP File Names to UTF-8 (ZIFU)", version = crate_version!(), author = crate_authors!(), about = crate_description!())]
#[clap(setting = AppSettings::ColoredHelp)]
struct CLIOptions {
    #[clap(
        about = "Path to the ZIP file where you want to change the encoding of the file name to UTF-8"
    )]
    input: String,
    #[clap(about = "Path to output")]
    output: Option<String>,
    #[clap(
        short,
        long,
        about = "Finds out if its file names are encoded in UTF-8."
    )]
    check: bool,
    #[clap(
        short,
        long,
        about = "Displays the list of file names in the ZIP archive."
    )]
    list: bool,
    #[clap(short, long, about = "Don't show any messages. (implies -y)")]
    silent: bool,
    #[clap(short, long, about = "Don't show any messages. (implies -y)")]
    quiet: bool,
    #[clap(
        short,
        long,
        value_name = "ENCODING",
        about = "Specifies the encoding of file names in the ZIP archive."
    )]
    encoding: Option<String>,
    #[clap(
        short,
        long,
        about = "Treats the encoding of the ZIP archive as UTF-8 first. (Default: try legacy encoding first)"
    )]
    utf8: bool,
    #[clap(short, long, about = "Don't confirm")]
    yes: bool,
}

impl CLIOptions {
    pub fn to_behavior_flags(&self) -> BehaviorFlags {
        let verbose = !self.silent && !self.quiet;
        return BehaviorFlags {
            verbose,
            ask_user: verbose && !self.yes,
        };
    }
}

fn main() -> anyhow::Result<()> {
    let cli_options = CLIOptions::parse();

    let behavior_flags = cli_options.to_behavior_flags();
    let mut zip_file = BufReader::new(File::open(&cli_options.input)?);

    let mut eocd = ZipEOCD::from_reader(&mut zip_file)?;
    eocd.check_unsupported_zip_type()?;

    let mut cd_entries = ZipCDEntry::all_from_eocd(&mut zip_file, &eocd)?;

    if cli_options.check {
        let archive_names_type = check_archive(&eocd, &cd_entries)?;
        print_status_message(&archive_names_type);
        std::process::exit(if archive_names_type.is_universal_archive() {
            0
        } else {
            2
        });
    }

    let legacy_decoder = if let Some(encoding_name) = cli_options.encoding.as_deref() {
        <dyn filename_decoder::IDecoder>::from_encoding_name(encoding_name).ok_or(
            InvalidArgument::InvalidEncodingName {
                encoding_name: encoding_name.to_string(),
            },
        )?
    } else {
        <dyn filename_decoder::IDecoder>::native_oem_encoding()
    };
    let utf8_decoder = <dyn filename_decoder::IDecoder>::utf8();
    let ascii_decoder = <dyn filename_decoder::IDecoder>::ascii();
    let decoders_list = if cli_options.utf8 {
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

    if cli_options.list {
        list_names_in_archive(&cd_entries, &*utf8_decoder, &**guessed_encoder);
        return Ok(());
    }
    if behavior_flags.verbose || behavior_flags.ask_user {
        list_names_in_archive(&cd_entries, &*utf8_decoder, &**guessed_encoder);
        if behavior_flags.ask_user {
            eprint!("Are these file names correct? [Y/n]: ");
            if !(ask_default_yes()?) {
                std::process::exit(1);
            }
        }
    }
    let output_zip_file_str = cli_options
        .output
        .as_ref()
        .ok_or(InvalidArgument::NoArgument {
            arg_name: "output".to_string(),
        })?;
    if &(cli_options.input) == output_zip_file_str {
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
        let cli_options = CLIOptions::parse_from(vec!["zifu", "before.zip", "after.zip"]);
        let global_flags = cli_options.to_behavior_flags();

        assert_eq!(global_flags.ask_user, true);
        assert_eq!(global_flags.verbose, true);

        assert_eq!(cli_options.input, "before.zip");
        assert_eq!(cli_options.output.as_deref(), Some("after.zip"));
    }

    #[test]
    fn extended_args_parse_test1() {
        let cli_options =
            CLIOptions::parse_from(vec!["zifu", "before.zip", "after.zip", "-q", "-u", "-l"]);
        let global_flags = cli_options.to_behavior_flags();

        assert_eq!(global_flags.ask_user, false);
        assert_eq!(global_flags.verbose, false);

        assert_eq!(cli_options.input, "before.zip");
        assert_eq!(cli_options.output.as_deref(), Some("after.zip"));
        assert_eq!(cli_options.encoding.as_deref(), None);
        assert_eq!(cli_options.utf8, true);
        assert_eq!(cli_options.check, false);
        assert_eq!(cli_options.list, true);
    }

    #[test]
    fn extended_args_parse_test2() {
        let cli_options = CLIOptions::parse_from(vec![
            "zifu",
            "before.zip",
            "after.zip",
            "-s",
            "-e",
            "sjis",
            "-c",
        ]);
        let global_flags = cli_options.to_behavior_flags();

        assert_eq!(global_flags.ask_user, false);
        assert_eq!(global_flags.verbose, false);

        assert_eq!(cli_options.input, "before.zip");
        assert_eq!(cli_options.output.as_deref(), Some("after.zip"));
        assert_eq!(cli_options.encoding.as_deref(), Some("sjis"));
        assert_eq!(cli_options.utf8, false);
        assert_eq!(cli_options.check, true);
        assert_eq!(cli_options.list, false);
    }

    #[test]
    fn extended_args_parse_test3() {
        let cli_options = CLIOptions::parse_from(vec![
            "zifu",
            "before.zip",
            "after.zip",
            "-y",
            "--encoding",
            "cp437",
        ]);
        let global_flags = cli_options.to_behavior_flags();

        assert_eq!(global_flags.ask_user, false);
        assert_eq!(global_flags.verbose, true);

        assert_eq!(cli_options.input, "before.zip");
        assert_eq!(cli_options.output.as_deref(), Some("after.zip"));
        assert_eq!(cli_options.encoding.as_deref(), Some("cp437"));
        assert_eq!(cli_options.utf8, false);
        assert_eq!(cli_options.check, false);
        assert_eq!(cli_options.list, false);
    }
}
