mod filename_decoder;
mod zip_central_directory;
mod zip_eocd;
mod zip_error;
use ansi_term::Color::{Green, Red};
use anyhow::anyhow;
use clap::{App, Arg};
use std::fs::File;
use std::io::BufReader;
use zip_central_directory::ZipCDEntry;
use zip_eocd::ZipEOCD;

#[derive(thiserror::Error, Debug)]
enum InvalidArgument {
    #[error("no argument <{arg_name}> was passed")]
    NoArgument { arg_name: String },
    #[error("unknown encoding name: {encoding_name}")]
    InvalidEncodingName { encoding_name: String },
}

fn main() -> anyhow::Result<()> {
    let app = App::new("zfu")
        .arg(
            Arg::with_name("input")
                .about("Path to the ZIP file where you want to change the encoding of the file name to UTF-8")
                .required(true)
            )
        .arg(
            Arg::with_name("output")
                .about("Path to output")
        )
        .arg(
            Arg::with_name("check")
                .long("check")
                .short('c')
                .about("Finds out if its file names are encoded in UTF-8.")
        )
        .arg(
            Arg::with_name("list")
                .short('l')
                .long("list")
                .about("Displays the list of file names in the ZIP archive.")
        )
        .arg(
            Arg::with_name("encoding")
            .long("encoding")
            .short('e')
            .value_name("ENCODING")
            .about("Specifies the encoding of file names in the ZIP archive.")
        )
        .arg(
            Arg::with_name("utf-8")
                .long("utf8")
                .short('u')
                .about("Treats the encoding of the ZIP archive as UTF-8 first. (Default: try legacy encoding first)")
        );

    let matches = app.get_matches();
    let mut zip_file = match matches.value_of("input") {
        None => {
            return Err(InvalidArgument::NoArgument {
                arg_name: "input".to_string(),
            }
            .into());
        }
        Some(a) => BufReader::new(File::open(a)?),
    };

    let eocd = ZipEOCD::from_reader(&mut zip_file)?;
    eocd.check_unsupported_zip_type()?;

    let cd_entries = ZipCDEntry::all_from_eocd(&mut zip_file, &eocd)?;

    if matches.is_present("check") {
        let utf8_entries_count = cd_entries
            .iter()
            .filter(|&cd| cd.is_encoded_in_utf8())
            .count();
        if utf8_entries_count == eocd.n_cd_entries as usize {
            println!(
                "{}",
                Green
                    .bold()
                    .paint("All file names are explicitly encoded in UTF-8.")
            );
            return Ok(());
        }
        if utf8_entries_count > 0 {
            println!(
                "{}",
                Red.bold().paint(format!(
                    "Some file names are not explicitly encoded in UTF-8. ({} / {})",
                    eocd.n_cd_entries as usize - utf8_entries_count,
                    eocd.n_cd_entries
                ))
            );
            std::process::exit(1);
        }
        println!(
            "{}",
            Red.bold()
                .paint("All file names are not explicitly encoded in UTF-8.")
        );
        std::process::exit(1);
    }

    let legacy_decoder = if let Some(encoding_name) = matches.value_of("encoding") {
        filename_decoder::IDecoder::from_encoding_name(encoding_name).ok_or(
            InvalidArgument::InvalidEncodingName {
                encoding_name: encoding_name.to_string(),
            },
        )?
    } else {
        filename_decoder::IDecoder::windows_legacy_encoding()
    };
    let utf8_decoder = filename_decoder::IDecoder::utf8();
    let decoders_list = if matches.is_present("utf-8") {
        vec![&utf8_decoder, &legacy_decoder]
    } else {
        vec![&legacy_decoder, &utf8_decoder]
    };
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
                    guessed_encoder
                        .color()
                        .bold()
                        .paint(guessed_encoder.encoding_name()),
                    guessed_encoder.to_string_lossy(&cd.file_name_raw)
                );
            }
        }
    } else {
        return Err(anyhow!("Sorry, correction mode is not yet implemented. Please try again with -c or -l option."));
    }
    return Ok(());
}
