use super::zip_central_directory::{ZipCDEntry, DATA_DESCRIPTOR_EXISTS_FLAG_BIT, UTF8_FLAG_BIT};
use super::zip_error::ZipReadError;
use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use std::io::prelude::*;
use std::io::SeekFrom;

const LOCAL_FILE_MAGIC: [u8; 4] = [0x50, 0x4b, 0x3, 0x4];

pub struct ZipDataDescriptor {
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
}

impl ZipDataDescriptor {
    fn empty() -> Self {
        return Self {
            crc32: 0,
            compressed_size: 0,
            uncompressed_size: 0,
        };
    }
    fn from_reader<T: ReadBytesExt>(read: &mut T) -> Result<Self, ZipReadError> {
        let mut result = Self::empty();
        result.crc32 = read.read_u32::<LE>()?;
        result.compressed_size = read.read_u32::<LE>()?;
        result.uncompressed_size = read.read_u32::<LE>()?;
        return Ok(result);
    }
    fn write<T: WriteBytesExt>(&self, write: &mut T) -> std::io::Result<u64> {
        write.write_u32::<LE>(self.crc32)?;
        write.write_u32::<LE>(self.compressed_size)?;
        write.write_u32::<LE>(self.uncompressed_size)?;
        return Ok(12);
    }
}

pub struct ZipLocalFileHeader {
    pub version_required_to_extract: u16,
    pub general_purpose_flags: u16,
    pub compression_method: u16,
    pub last_mod_time: u16,
    pub last_mod_date: u16,
    pub crc32: u32,
    pub compressed_size: u32,
    pub uncompressed_size: u32,
    pub file_name_length: u16,
    pub extra_field_length: u16,
    pub file_name_raw: Vec<u8>,
    pub extra_field: Vec<u8>,
    pub compressed_data: Vec<u8>,
    pub data_descriptor: Option<ZipDataDescriptor>,
    pub starting_position_with_signature: u64,
    pub starting_position_without_signature: u64,
}

impl ZipLocalFileHeader {
    fn empty() -> Self {
        return Self {
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
            file_name_raw: vec![],
            extra_field: vec![],
            compressed_data: vec![],
            data_descriptor: None,
            starting_position_with_signature: 0,
            starting_position_without_signature: 0,
        };
    }

    fn read_without_signature<T: ReadBytesExt + std::io::Seek>(
        &mut self,
        read: &mut T,
    ) -> Result<(), ZipReadError> {
        self.starting_position_without_signature = read.seek(SeekFrom::Current(0))?;
        self.starting_position_with_signature =
            self.starting_position_without_signature - LOCAL_FILE_MAGIC.len() as u64;
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
        let read_compressed_size = read
            .take(self.compressed_size as u64)
            .read_to_end(&mut self.compressed_data)?;
        if read_compressed_size != self.compressed_size as usize {
            return Err(ZipReadError::InvalidZipArchive {
                reason: format!(
                    "compressed size is invalid (expected from length value field: {} / got {}",
                    self.compressed_size, read_compressed_size
                ),
            });
        }
        if self.has_data_descriptor_by_flag() {
            self.data_descriptor = Some(ZipDataDescriptor::from_reader(read)?);
        }
        return Ok(());
    }

    pub fn set_utf8_encoded_flag(&mut self) {
        self.general_purpose_flags |= UTF8_FLAG_BIT;
    }
    pub fn set_file_name_from_slice(&mut self, name: &Vec<u8>) {
        self.file_name_length = name.len() as u16;
        self.file_name_raw.clone_from(name);
    }

    fn has_data_descriptor_by_flag(&self) -> bool {
        return (DATA_DESCRIPTOR_EXISTS_FLAG_BIT & self.general_purpose_flags) != 0;
    }

    pub fn from_central_directory<T: ReadBytesExt + std::io::Seek>(
        read: &mut T,
        cd: &ZipCDEntry,
    ) -> Result<Self, ZipReadError> {
        read.seek(SeekFrom::Start(cd.local_header_position as u64))?;
        let mut signature_candidate: [u8; 4] = [0; 4];
        let start_pos = read.seek(SeekFrom::Current(0))?;
        read.read_exact(&mut signature_candidate)?;
        if signature_candidate != LOCAL_FILE_MAGIC {
            return Err(ZipReadError::InvalidZipArchive {
                reason: format!(
                    "assumed local file header signature doesn't appear at position {}",
                    start_pos
                ),
            });
        }
        let mut ret = Self::empty();
        ret.read_without_signature(read)?;
        return Ok(ret);
    }
    pub fn write<T: WriteBytesExt>(&self, write: &mut T) -> std::io::Result<u64> {
        let mut bytes_written = 30
            + self.file_name_length as u64
            + self.extra_field_length as u64
            + self.compressed_size as u64;
        write.write_all(&LOCAL_FILE_MAGIC)?;
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
        write.write_all(self.file_name_raw.as_slice())?;
        write.write_all(self.extra_field.as_slice())?;
        write.write_all(self.compressed_data.as_slice())?;
        if self.data_descriptor.is_some() {
            bytes_written += self.data_descriptor.as_ref().unwrap().write(write)?;
        }
        return Ok(bytes_written);
    }
}
