mod filename_decoder;
mod zip_central_directory;
mod zip_eocd;
mod zip_error;
mod zip_local_file_header;
use ansi_term::Color::{Green, Red};
use anyhow::anyhow;
use clap::{App, Arg};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use zip_central_directory::ZipCDEntry;
use zip_eocd::ZipEOCD;

#[derive(thiserror::Error, Debug)]
enum InvalidArgument {
    #[error("no argument <{arg_name}> was passed")]
    NoArgument { arg_name: String },
    #[error("unknown encoding name: {encoding_name}")]
    InvalidEncodingName { encoding_name: String },
    #[error("you cannot specify the same file for input and output files.")]
    SameInputOutput,
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

    let mut eocd = ZipEOCD::from_reader(&mut zip_file)?;
    eocd.check_unsupported_zip_type()?;

    let mut cd_entries = ZipCDEntry::all_from_eocd(&mut zip_file, &eocd)?;

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
    let ascii_decoder = filename_decoder::IDecoder::ascii();
    let decoders_list = if matches.is_present("utf-8") {
        vec![&ascii_decoder, &utf8_decoder, &legacy_decoder]
    } else {
        vec![&ascii_decoder, &legacy_decoder, &utf8_decoder]
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
        let output_zip_file_str =
            matches
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
        let mut pos: u64 = 0;
        for cd in &mut cd_entries {
            let mut local_header =
                zip_local_file_header::ZipLocalFileHeader::from_central_directory(
                    &mut zip_file,
                    cd,
                )?;
            if !cd.is_encoded_in_utf8() {
                let file_name_u8 = guessed_encoder.to_string_lossy(&cd.file_name_raw);
                let file_name_u8bytes = file_name_u8.as_bytes().to_vec();

                cd.set_file_name_from_slice(&file_name_u8bytes);
                local_header.set_file_name_from_slice(&file_name_u8bytes);
                let file_comment_u8 = guessed_encoder.to_string_lossy(&cd.file_comment);
                let file_comment_u8bytes = file_comment_u8.as_bytes().to_vec();
                cd.set_file_coment_from_slice(&file_comment_u8bytes);
                cd.set_utf8_encoded_flag();
                local_header.set_utf8_encoded_flag();
            }
            cd.local_header_position = pos as u32;
            pos += local_header.write(&mut output_zip_file)?;
        }
        eocd.cd_starting_position = pos as u32;
        let mut cd_new_size: u64 = 0;
        for cd in cd_entries {
            cd_new_size += cd.write(&mut output_zip_file)?;
        }
        eocd.cd_size = cd_new_size as u32;
        eocd.write(&mut output_zip_file)?;
    }
    return Ok(());
}
