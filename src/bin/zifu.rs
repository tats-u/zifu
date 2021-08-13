use ansi_term::Color::{Green, Red};
use anyhow::anyhow;
use clap::{crate_authors, crate_description, crate_version, AppSettings, Clap};
use filename_decoder::IDecoder;
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::vec;
use zifu::{filename_decoder, FileNameEntry, ZIFURequirement};
use zifu::{InputZIPArchive, ZipFileEncodingType};

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

/// Decodes and prints file names in central directories to stdout
///
/// # Arguments
///
/// * `cd_entries` - Central directories (contains file names)
/// * `utf8_decoder` - UTF-8 decoder (used when explicitly encoded in UTF-8)
/// * `legacy_decoder` - Legacy charset decoder (used otherwise)
fn list_names_in_archive(fie_name_entries: &[FileNameEntry], legacy_decoder: &dyn IDecoder) {
    for entry in fie_name_entries {
        if entry.is_encoding_explicit {
            println!(
                "{}:{}:{}",
                Green.bold().paint("EXPLICIT"),
                Green.bold().paint("UTF-8"),
                &entry.name
            );
        } else {
            println!(
                "{}:{}:{}",
                Red.bold().paint("GUESSED"),
                legacy_decoder
                    .color()
                    .bold()
                    .paint(legacy_decoder.encoding_name()),
                &entry.name
            );
        }
    }
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
    let mut input_zip_file = InputZIPArchive::new(BufReader::new(File::open(&cli_options.input)?))?;

    input_zip_file.check_unsupported_zip_type()?;

    if cli_options.check {
        let archive_names_type = input_zip_file.check_file_name_encoding();
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
        vec![&*ascii_decoder, &*utf8_decoder, &*legacy_decoder]
    } else {
        vec![&*ascii_decoder, &*legacy_decoder, &*utf8_decoder]
    };
    // Detect encoding by trying decoding all of file names and comments
    let best_fit_decoder_index_ = input_zip_file.get_filename_decoder_index(&decoders_list);
    best_fit_decoder_index_.ok_or(anyhow!(
        "file names & comments are not encoded in UTF-8 or {}.  Try with -e <another encoding> option.",
        legacy_decoder.encoding_name()
    ))?;
    let guessed_encoder = decoders_list[best_fit_decoder_index_.unwrap()];

    if cli_options.list {
        list_names_in_archive(
            &input_zip_file.get_file_names_list(guessed_encoder),
            guessed_encoder,
        );
        return Ok(());
    }
    if behavior_flags.verbose || behavior_flags.ask_user {
        list_names_in_archive(
            &input_zip_file.get_file_names_list(guessed_encoder),
            guessed_encoder,
        );

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

    input_zip_file.convert_central_directory_file_names(guessed_encoder);
    let mut output_zip_file = BufWriter::new(File::create(output_zip_file_str)?);
    input_zip_file.output_archive_with_central_directory_file_names(&mut output_zip_file)?;
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
