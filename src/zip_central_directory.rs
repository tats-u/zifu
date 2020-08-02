use super::zip_eocd::ZipEOCD;
use super::zip_error::ZipReadError;
use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use bytesize::ByteSize;
use std::io::prelude::*;
use std::io::SeekFrom;

/// Magic number of central directory
const CD_MAGIC: [u8; 4] = [0x50, 0x4b, 0x1, 0x2];

/// bit #0 (0x0001 = 1 << 0) of general purpose bit flag
pub const DATA_ENCRYPTED_FLAG_BIT: u16 = 0x0001;
/// bit #3 (0x0008 = 1 << 3) of general purpose bit flag
pub const DATA_DESCRIPTOR_EXISTS_FLAG_BIT: u16 = 0x0008;
/// bit #11 (0x0800 = 1 << 11) of general purpose bit flag
pub const UTF8_FLAG_BIT: u16 = 0x0800;

/// ZIPファイルのセントラルディレクトリの1エントリー
/// An entry of central directory of ZIP file
pub struct ZipCDEntry {
    /// As the name implies; see 4.4.2 in https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT
    ///
    /// Unaffected by file renaming
    pub version_made_by: u16,
    /// As the name implies; see 4.4.3 in https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT
    ///
    /// Unaffected by file renaming
    pub version_required_to_extract: u16,
    /// See 4.4.4 in https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT
    ///
    /// bit #n reprents 1 << n in little endian
    ///
    /// Unaffected by file renaming
    pub general_purpose_flags: u16,
    /// As the name implies; see 4.4.5 in https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT
    ///
    /// Unaffected by file renaming
    pub compression_method: u16,
    /// As the name implies; see 4.4.6 in https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT
    ///
    /// MS-DOS time: http://www.ffortune.net/calen/calen/etime.htm (Japanese)
    ///
    /// Unaffected by file renaming
    pub last_mod_time: u16,
    /// As the name implies; see 4.4.6 in https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT
    ///
    /// MS-DOS time: http://www.ffortune.net/calen/calen/etime.htm (Japanese)
    ///
    /// Unaffected by file renaming
    pub last_mod_date: u16,
    /// See 4.4.7 in https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT
    ///
    /// Unaffected by file renaming
    pub crc32: u32,
    /// As the name implies.  Note that the file name is not included.
    pub compressed_size: u32,
    /// As the name implies.  Note that the file name is not included.
    pub uncompressed_size: u32,
    /// As the name implies.
    pub file_name_length: u16,
    /// As the name implies.
    pub extra_field_length: u16,
    /// As the name implies.
    pub file_comment_length: u16,
    /// the number (0-baesd) of the disk where the file for this central directory is.
    ///
    /// Unaffected by file renaming
    pub disk_number_start: u16,
    /// See 4.4.14 in https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT
    ///
    /// Unaffected by file renaming
    pub internal_file_attributes: u16,
    /// See 4.4.15 in https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT
    ///
    /// Unaffected by file renaming
    pub external_file_attributes: u32,
    /// **Absolute** 0-based position of the local header for this central directory
    pub local_header_position: u32,
    /// Byte sequence of the file name.
    pub file_name_raw: Vec<u8>,
    /// Byte sequence of extra field
    pub extra_field: Vec<u8>,
    /// File comment; must be encoded in the same encoding as the file name.
    pub file_comment: Vec<u8>,

    // セントラルディレクトリのエントリここまで / End of central directory entries
    /// セントラルディレクトリの開始位置 (マジックナンバー) /
    /// (magick number of) central directory starting position
    pub starting_position_with_signature: u64,
    /// セントラルディレクトリの開始位置 (マジックナンバーすぐ次) /
    /// Central directory starting position (next to magick number)
    pub starting_position_without_signature: u64,
}

impl ZipCDEntry {
    ///空のセントラルディレクトリオブジェクトを生成 /
    /// Generates an empty central directory object
    fn empty() -> Self {
        return Self {
            version_made_by: 0,
            version_required_to_extract: 0,
            general_purpose_flags: 0,
            compression_method: 0,
            last_mod_time: 0,
            last_mod_date: 0,
            crc32: 0,
            compressed_size: 0,
            uncompressed_size: 0,
            file_name_length: 0,
            extra_field_length: 0,
            file_comment_length: 0,
            disk_number_start: 0,
            internal_file_attributes: 0,
            external_file_attributes: 0,
            local_header_position: 0,
            file_name_raw: vec![],
            extra_field: vec![],
            file_comment: vec![],
            starting_position_with_signature: 0,
            starting_position_without_signature: 0,
        };
    }

    /// Reads from next to the signature (magick number) of the central directory.
    ///
    /// # Arguments
    /// * `read` - `Read` object (must be at the next to the signature)
    fn read_from_eocd_next_signature<T: ReadBytesExt + std::io::Seek>(
        &mut self,
        read: &mut T,
    ) -> Result<(), ZipReadError> {
        self.starting_position_without_signature = read.seek(SeekFrom::Current(0))?;
        self.starting_position_with_signature =
            self.starting_position_without_signature - CD_MAGIC.len() as u64;
        self.version_made_by = read.read_u16::<LE>()?;
        self.version_required_to_extract = read.read_u16::<LE>()?;
        self.general_purpose_flags = read.read_u16::<LE>()?;
        self.compression_method = read.read_u16::<LE>()?;
        self.last_mod_time = read.read_u16::<LE>()?;
        self.last_mod_date = read.read_u16::<LE>()?;
        self.crc32 = read.read_u32::<LE>()?;
        self.compressed_size = read.read_u32::<LE>()?;
        self.uncompressed_size = read.read_u32::<LE>()?;
        self.file_name_length = read.read_u16::<LE>()?;
        self.extra_field_length = read.read_u16::<LE>()?;
        self.file_comment_length = read.read_u16::<LE>()?;
        self.disk_number_start = read.read_u16::<LE>()?;
        self.internal_file_attributes = read.read_u16::<LE>()?;
        self.external_file_attributes = read.read_u32::<LE>()?;
        self.local_header_position = read.read_u32::<LE>()?;
        self.check_unsupported()?;
        let read_file_name_length = read
            .take(self.file_name_length as u64)
            .read_to_end(&mut self.file_name_raw)?;
        if read_file_name_length != self.file_name_length as usize {
            return Err(ZipReadError::InvalidZipArchive {
                reason: format!(
                    "file name length is invalid (expected from length value field: {} / got: {})",
                    self.file_name_length, read_file_name_length
                ),
            });
        }
        let read_extra_field_length = read
            .take(self.extra_field_length as u64)
            .read_to_end(&mut self.extra_field)?;
        if read_extra_field_length != self.extra_field_length as usize {
            return Err(ZipReadError::InvalidZipArchive {
                reason: format!(
                    "extra field length is invalid (expected from length value field: {} / got {}",
                    self.extra_field_length, read_extra_field_length
                ),
            });
        }
        let read_file_comment_length = read
            .take(self.file_comment_length as u64)
            .read_to_end(&mut self.file_comment)?;
        if read_file_comment_length != self.file_comment_length as usize {
            return Err(ZipReadError::InvalidZipArchive {
                reason: format!(
                    "file comment length is invalid (expected from length value field: {} / got {}",
                    self.file_comment_length, read_file_comment_length
                ),
            });
        }
        return Ok(());
    }
    /// Sets bit #11 of general purpose bit to indicate that the file name & comment are encoded in UTF-8.
    pub fn set_utf8_encoded_flag(&mut self) {
        self.general_purpose_flags |= UTF8_FLAG_BIT;
    }
    /// Replaces the file name.
    ///
    /// # Arguments
    ///
    /// * `name` - Slice of new name
    pub fn set_file_name_from_slice(&mut self, name: &Vec<u8>) {
        self.file_name_length = name.len() as u16;
        self.file_name_raw.clone_from(name);
    }
    /// Replaces the file comment
    ///
    /// # Arguments
    ///
    /// * `comment` - Slice of new comment
    pub fn set_file_coment_from_slice(&mut self, comment: &Vec<u8>) {
        self.file_comment_length = comment.len() as u16;
        self.file_comment.clone_from(comment);
    }
    /// Returns whether the file name and comment are explicitly encoded in UTF-8
    pub fn is_encoded_in_utf8(&self) -> bool {
        return (UTF8_FLAG_BIT & self.general_purpose_flags) != 0;
    }
    /// Returns whether the file content is encrypted
    pub fn is_encrypted_data(&self) -> bool {
        return (DATA_ENCRYPTED_FLAG_BIT & self.general_purpose_flags) != 0;
    }
    /// Returns `Error` if the file and central directory have unsupported features
    pub fn check_unsupported(&self) -> Result<(), ZipReadError> {
        if self.disk_number_start != 0 {
            return Err(ZipReadError::UnsupportedZipArchive {
                reason: "it is one of splitted arvhives".to_string(),
            });
        }
        if self.is_encrypted_data() {
            return Err(ZipReadError::UnsupportedZipArchive {
                reason: "encrypted data is not supported".to_string(),
            });
        }
        return Ok(());
    }
    /// Writes the content of this central directory to file and returns the number of bytes written.
    ///
    /// # Arguments
    ///
    /// * `write` - file handler
    pub fn write<T: WriteBytesExt>(&self, write: &mut T) -> std::io::Result<u64> {
        write.write_all(&CD_MAGIC)?;
        write.write_u16::<LE>(self.version_made_by)?;
        write.write_u16::<LE>(self.version_required_to_extract)?;
        write.write_u16::<LE>(self.general_purpose_flags)?;
        write.write_u16::<LE>(self.compression_method)?;
        write.write_u16::<LE>(self.last_mod_time)?;
        write.write_u16::<LE>(self.last_mod_date)?;
        write.write_u32::<LE>(self.crc32)?;
        write.write_u32::<LE>(self.compressed_size)?;
        write.write_u32::<LE>(self.uncompressed_size)?;
        write.write_u16::<LE>(self.file_name_length)?;
        write.write_u16::<LE>(self.extra_field_length)?;
        write.write_u16::<LE>(self.file_comment_length)?;
        write.write_u16::<LE>(self.disk_number_start)?;
        write.write_u16::<LE>(self.internal_file_attributes)?;
        write.write_u32::<LE>(self.external_file_attributes)?;
        write.write_u32::<LE>(self.local_header_position)?;
        write.write_all(self.file_name_raw.as_slice())?;
        write.write_all(self.extra_field.as_slice())?;
        write.write_all(self.file_comment.as_slice())?;
        return Ok(46
            + self.file_name_length as u64
            + self.extra_field_length as u64
            + self.file_comment_length as u64);
    }
    /// Examines the signature, reads the central directory and returns an instance that represents it
    ///
    /// # Arguments
    ///
    /// * `read` - file handler (must be at the head of the signature)
    fn read_and_generate_from_signature<T: ReadBytesExt + std::io::Seek>(
        read: &mut T,
    ) -> Result<Self, ZipReadError> {
        let mut signature_candidate: [u8; 4] = [0; 4];
        let start_pos = read.seek(SeekFrom::Current(0))?;
        read.read_exact(&mut signature_candidate)?;
        if signature_candidate != CD_MAGIC {
            return Err(ZipReadError::InvalidZipArchive {
                reason: format!(
                    "assumed central directry signature doesn't appear at position {}",
                    start_pos
                ),
            });
        }
        let mut result = Self::empty();
        result.read_from_eocd_next_signature(read)?;
        return Ok(result);
    }
    /// Reads and returns a central directory sequence from the given EOCD
    ///
    /// # Arguments
    ///
    /// * `read` - file handler
    /// * `eocd` - EOCD object
    pub fn all_from_eocd<T: ReadBytesExt + std::io::Seek>(
        mut read: &mut T,
        eocd: &ZipEOCD,
    ) -> Result<Vec<Self>, ZipReadError> {
        read.seek(SeekFrom::Start(eocd.cd_starting_position as u64))?;
        let mut result: Vec<Self> = vec![];
        for _ in 0..eocd.n_cd_entries {
            result.push(Self::read_and_generate_from_signature(&mut read)?);
        }
        let end_pos = read.seek(SeekFrom::Current(0))?;
        if end_pos != eocd.starting_position_with_signature {
            return Err(ZipReadError::UnsupportedZipArchive {
                reason: format!("there are extra data ({}) between central directory and end of central directory", ByteSize::b(eocd.starting_position_with_signature - end_pos))
            });
        }
        return Ok(result);
    }
}
