use std::{
    fs::File,
    io::Cursor,
    io::{BufReader, BufWriter, Read, Seek, SeekFrom},
    path::PathBuf,
    process::Command,
};

use tempfile::tempdir;
use zifu::{
    filename_decoder::{self, IDecoder},
    InputZIPArchive, ZipFileEncodingType,
};

fn open_bufreader(path: &str) -> anyhow::Result<BufReader<File>> {
    return Ok(BufReader::new(File::open(path)?));
}

fn read_all(f: &mut dyn Read) -> Result<Vec<u8>, std::io::Error> {
    let mut buf = Vec::<u8>::new();
    f.read_to_end(&mut buf).map(|_| buf)
}

/// passord is fixed at `test`
fn test_command_7z(path: &PathBuf) -> Command {
    let mut cmd = Command::new("7z");
    cmd.args(&["t", "-ptest"]).arg(path.as_os_str());
    return cmd;
}

#[test]
fn convert_and_compare_content_test() -> anyhow::Result<()> {
    let mut before = InputZIPArchive::new(open_bufreader("tests/assets/before.zip")?)?;
    before.check_unsupported_zip_type()?;
    assert!(
        matches!(
            before.check_file_name_encoding(),
            ZipFileEncodingType::AllLegacy
        ),
        "file name encoding == AllLegacy"
    );
    let sjis_decoder = <dyn filename_decoder::IDecoder>::from_encoding_name("sjis").ok_or(
        anyhow::anyhow!("`sjis` is not suitable encoding name for `IDecoder::from_encoding_name`"),
    )?;
    assert!(
        matches!(
            before.get_filename_decoder_index(&vec![&*sjis_decoder]),
            Some(_)
        ),
        "sjis decoder is matched",
    );
    let names_list = before.get_file_names_list(&*sjis_decoder);
    let name_entry = names_list
        .get(0)
        .ok_or(anyhow::anyhow!("`names_list` has at least one entry"))?;
    assert_eq!(name_entry.name, "テスト.txt", "file name is `テスト.txt`");
    assert_eq!(
        name_entry.is_encoding_explicit, false,
        "file name is implicitly encoded"
    );

    before.convert_central_directory_file_names(&*sjis_decoder);

    assert!(
        matches!(
            before.check_file_name_encoding(),
            ZipFileEncodingType::AllExplicitUTF8
        ),
        "file name encoding turned to be AllExplicitUTF8"
    );
    let names_list = before.get_file_names_list(&*sjis_decoder);
    let name_entry = names_list
        .get(0)
        .ok_or(anyhow::anyhow!("`names_list` still has at least one entry"))?;
    assert_eq!(
        name_entry.name, "テスト.txt",
        "file name is still `テスト.txt`"
    );
    assert_eq!(
        name_entry.is_encoding_explicit, true,
        "file name turned to be explicitly encoded"
    );

    let mut dump = Cursor::new(Vec::<u8>::new());
    before.output_archive_with_central_directory_file_names(&mut dump)?;
    dump.seek(SeekFrom::Start(0))?;
    let mut after = File::open("tests/assets/after.zip")?;
    assert_eq!(
        read_all(&mut dump)?,
        read_all(&mut after)?,
        "Dumped content is the same as what is expected (`after.zip`)"
    );

    Ok(())
}

#[test]
fn utf8_unencrypted_archive_test() -> anyhow::Result<()> {
    let mut zip = InputZIPArchive::new(open_bufreader("tests/assets/after.zip")?)?;
    zip.check_unsupported_zip_type()?;
    assert!(
        matches!(
            zip.check_file_name_encoding(),
            ZipFileEncodingType::AllExplicitUTF8
        ),
        "file name encoding is AllExplicitUTF8"
    );
    let decoder = <dyn IDecoder>::utf8();
    let names_list = zip.get_file_names_list(&*decoder);
    let name_entry = names_list
        .get(0)
        .ok_or(anyhow::anyhow!("`names_list` has at least one entry"))?;
    assert_eq!(name_entry.name, "テスト.txt", "file name is `テスト.txt`");
    assert_eq!(
        name_entry.is_encoding_explicit, true,
        "file name is explicitly encoded"
    );

    let mut dump1 = Cursor::new(Vec::<u8>::new());
    zip.output_archive_with_central_directory_file_names(&mut dump1)?;
    dump1.seek(SeekFrom::Start(0))?;

    zip.convert_central_directory_file_names(&*decoder);

    let mut dump2 = Cursor::new(Vec::<u8>::new());
    zip.output_archive_with_central_directory_file_names(&mut dump2)?;
    dump2.seek(SeekFrom::Start(0))?;

    assert_eq!(
        read_all(&mut dump1)?,
        read_all(&mut dump2)?,
        "content not changed"
    );

    Ok(())
}

#[test]
fn zipcrypto_convert_test() -> anyhow::Result<()> {
    let mut before = InputZIPArchive::new(open_bufreader("tests/assets/zipcrypto_sjis.zip")?)?;
    before.check_unsupported_zip_type()?;
    assert!(
        matches!(
            before.check_file_name_encoding(),
            ZipFileEncodingType::AllLegacy
        ),
        "file name encoding == AllLegacy"
    );
    let sjis_decoder = <dyn filename_decoder::IDecoder>::from_encoding_name("sjis").ok_or(
        anyhow::anyhow!("`sjis` is not suitable encoding name for `IDecoder::from_encoding_name`"),
    )?;
    assert!(
        matches!(
            before.get_filename_decoder_index(&vec![&*sjis_decoder]),
            Some(_)
        ),
        "sjis decoder is matched",
    );
    let names_list = before.get_file_names_list(&*sjis_decoder);
    let name_entry = names_list
        .get(0)
        .ok_or(anyhow::anyhow!("`names_list` has at least one entry"))?;
    assert_eq!(name_entry.name, "テスト.txt", "file name is `テスト.txt`");
    assert_eq!(
        name_entry.is_encoding_explicit, false,
        "file name is implicitly encoded"
    );

    before.convert_central_directory_file_names(&*sjis_decoder);

    assert!(
        matches!(
            before.check_file_name_encoding(),
            ZipFileEncodingType::AllExplicitUTF8
        ),
        "file name encoding turned to be AllExplicitUTF8"
    );
    let names_list = before.get_file_names_list(&*sjis_decoder);
    let name_entry = names_list
        .get(0)
        .ok_or(anyhow::anyhow!("`names_list` still has at least one entry"))?;
    assert_eq!(
        name_entry.name, "テスト.txt",
        "file name is still `テスト.txt`"
    );
    assert_eq!(
        name_entry.is_encoding_explicit, true,
        "file name turned to be explicitly encoded"
    );

    if which::which("7z").is_ok() {
        let working_dir = tempdir()?;
        let dump_path = working_dir.path().join("aes256_utf8.zip");
        let mut dump = BufWriter::new(File::create(&dump_path)?);
        before.output_archive_with_central_directory_file_names(&mut dump)?;
        drop(dump);
        test_command_7z(&dump_path).status()?;
    } else {
        let mut dump = Cursor::new(Vec::<u8>::new());
        before.output_archive_with_central_directory_file_names(&mut dump)?;
    }

    Ok(())
}

#[test]
fn aes256_convert_test() -> anyhow::Result<()> {
    let mut before = InputZIPArchive::new(open_bufreader("tests/assets/zipcrypto_sjis.zip")?)?;
    before.check_unsupported_zip_type()?;
    assert!(
        matches!(
            before.check_file_name_encoding(),
            ZipFileEncodingType::AllLegacy
        ),
        "file name encoding == AllLegacy"
    );
    let sjis_decoder = <dyn filename_decoder::IDecoder>::from_encoding_name("sjis").ok_or(
        anyhow::anyhow!("`sjis` is not suitable encoding name for `IDecoder::from_encoding_name`"),
    )?;
    assert!(
        matches!(
            before.get_filename_decoder_index(&vec![&*sjis_decoder]),
            Some(_)
        ),
        "sjis decoder is matched",
    );
    let names_list = before.get_file_names_list(&*sjis_decoder);
    let name_entry = names_list
        .get(0)
        .ok_or(anyhow::anyhow!("`names_list` has at least one entry"))?;
    assert_eq!(name_entry.name, "テスト.txt", "file name is `テスト.txt`");
    assert_eq!(
        name_entry.is_encoding_explicit, false,
        "file name is implicitly encoded"
    );

    before.convert_central_directory_file_names(&*sjis_decoder);

    assert!(
        matches!(
            before.check_file_name_encoding(),
            ZipFileEncodingType::AllExplicitUTF8
        ),
        "file name encoding turned to be AllExplicitUTF8"
    );
    let names_list = before.get_file_names_list(&*sjis_decoder);
    let name_entry = names_list
        .get(0)
        .ok_or(anyhow::anyhow!("`names_list` still has at least one entry"))?;
    assert_eq!(
        name_entry.name, "テスト.txt",
        "file name is still `テスト.txt`"
    );
    assert_eq!(
        name_entry.is_encoding_explicit, true,
        "file name turned to be explicitly encoded"
    );

    if which::which("7z").is_ok() {
        let working_dir = tempdir()?;
        let dump_path = working_dir.path().join("zipcrypto_utf8.zip");
        let mut dump = BufWriter::new(File::create(&dump_path)?);
        before.output_archive_with_central_directory_file_names(&mut dump)?;
        drop(dump);
        test_command_7z(&dump_path).status()?;
    } else {
        let mut dump = Cursor::new(Vec::<u8>::new());
        before.output_archive_with_central_directory_file_names(&mut dump)?;
    }

    Ok(())
}
