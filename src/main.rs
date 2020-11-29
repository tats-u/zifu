mod filename_decoder;
mod zip_central_directory;
mod zip_eocd;
mod zip_error;
mod zip_local_file_header;
use ansi_term::Color::{Green, Red, Yellow};
use anyhow::anyhow;
use byteorder::{ReadBytesExt, WriteBytesExt};
use clap::{crate_authors, crate_description, crate_version, App, Arg};
use filename_decoder::IDecoder;
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

fn check_archive(eocd: &ZipEOCD, cd_entries: &Vec<ZipCDEntry>) -> anyhow::Result<bool> {
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
        return Ok(true);
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
        println!(
                "{}  {}",
                Yellow.bold().paint(
                    if utf8_entries_count > 0 {
                        format!(
                            "{} file names are explicitly encoded in UTF-8, and {} file names are implicitly ASCII.",
                            utf8_entries_count,
                            eocd.n_cd_entries as usize - utf8_entries_count,
                        )
                    } else {
                        "All file names are implicitly encoded in ASCII.".to_string()
                    }),
                Green.bold().paint("They can be extracted correctly in all environments without garbling.")
            );
        return Ok(true);
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
        return Ok(false);
    }
    println!(
        "{}",
        Red.bold()
            .paint("All file names are not explicitly encoded in UTF-8.")
    );
    return Ok(false);
}

fn list_names_in_archive(
    cd_entries: &Vec<ZipCDEntry>,
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
    return Ok(match ask_result.chars().next() {
        None | Some('n') | Some('N') => false,
        Some(_) => true,
    });
}

fn main() -> anyhow::Result<()> {
    let app = App::new("ZIP File Names to UTF-8 (ZIFU)")
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
            .about("Don't show any messages. (implies -y")
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

    let matches = app.get_matches();
    let verbose = !matches.is_present("silent") && !matches.is_present("quiet");
    let ask_user = verbose && !matches.is_present("yes");
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
        let is_arhive_safe = check_archive(&eocd, &cd_entries)?;
        std::process::exit(if is_arhive_safe { 0 } else { 2 });
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
    if verbose || ask_user {
        list_names_in_archive(&cd_entries, &*utf8_decoder, &**guessed_encoder);
        if ask_user {
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
