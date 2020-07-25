use super::zip_error::ZipReadError;
use byteorder::{ReadBytesExt, WriteBytesExt, LE};
use std::io::prelude::*;
use std::io::SeekFrom;

const EOCD_MAGIC: [u8; 4] = [0x50, 0x4b, 0x5, 0x6];

/// EOCD (End of Central Directory) 情報を保持する構造体
pub struct ZipEOCD {
    /// EOCDが存在するディスク番号 (0起算)
    pub eocd_disk_index: u16,
    /// セントラルディレクトリが始まるディスク番号 (0起算)
    pub cd_start_disk_index: u16,
    /// EOCDがあるディスク内のセントラルディレクトリ総数
    pub n_cd_entries_in_disk: u16,
    /// セントラルディレクトリ総数
    pub n_cd_entries: u16,
    /// セントラルディレクトリのサイズ
    pub cd_size: u32,
    /// セントラルディレクトリ開始位置 (絶対)
    pub cd_starting_position: u32,
    /// ZIPコメント長
    pub comment_length: u16,
    /// ZIPコメント
    pub comment: Vec<u8>,

    // EOCDのエントリここまで
    /// EOCDの開始位置 (マジックナンバー)
    pub starting_position_with_signature: u64,
    /// EOCDの開始位置 (マジックナンバーすぐ次)
    pub starting_position_without_signature: u64,
}

impl ZipEOCD {
    /// EOCDのマジックナンバーの次の文字が読み取り位置である`Read`オブジェクトから、EOCD情報オブジェクトを生成
    ///
    /// # Arguments
    ///
    /// * `read` - マジックナンバーの直後を指している`Read`オブジェクト
    /// * `pos` - マジックナンバーの直後のファイル位置 (デフォルト: 0)
    fn from_reader_next_to_signature<T: ReadBytesExt + std::io::Seek>(
        &mut self,
        read: &mut T,
    ) -> Result<bool, std::io::Error> {
        self.starting_position_without_signature = read.seek(SeekFrom::Current(0))?;
        self.starting_position_with_signature =
            self.starting_position_without_signature - EOCD_MAGIC.len() as u64;
        self.eocd_disk_index = read.read_u16::<LE>()?;
        self.cd_start_disk_index = read.read_u16::<LE>()?;
        self.n_cd_entries_in_disk = read.read_u16::<LE>()?;
        self.n_cd_entries = read.read_u16::<LE>()?;
        self.cd_size = read.read_u32::<LE>()?;
        self.cd_starting_position = read.read_u32::<LE>()?;
        self.comment_length = read.read_u16::<LE>()?;
        // + 1 for EOF detection
        let mut comment = read.take((self.comment_length as u64) + 1);
        self.comment.reserve(
            ((self.comment_length as usize) + 1)
                .checked_sub(self.comment.len())
                .unwrap_or(0),
        );
        let read_comment_length = comment.read_to_end(&mut self.comment)?;
        if read_comment_length != (self.comment_length as usize) {
            return Ok(false);
        }
        return Ok(true);
    }

    ///空のEOCDオブジェクトを生成
    fn empty() -> ZipEOCD {
        return ZipEOCD {
            eocd_disk_index: 0,
            cd_start_disk_index: 0,
            n_cd_entries_in_disk: 0,
            n_cd_entries: 0,
            cd_size: 0,
            cd_starting_position: 0,
            comment_length: 0,
            comment: vec![],
            starting_position_with_signature: 0,
            starting_position_without_signature: 0,
        };
    }

    pub fn write<T: WriteBytesExt>(&self, write: &mut T) -> std::io::Result<()> {
        write.write_all(&EOCD_MAGIC)?;
        write.write_u16::<LE>(self.eocd_disk_index)?;
        write.write_u16::<LE>(self.cd_start_disk_index)?;
        write.write_u16::<LE>(self.n_cd_entries_in_disk)?;
        write.write_u16::<LE>(self.n_cd_entries)?;
        write.write_u32::<LE>(self.cd_size)?;
        write.write_u32::<LE>(self.cd_starting_position)?;
        write.write_u16::<LE>(self.comment_length)?;
        write.write_all(self.comment.as_slice())?;
        return Ok(());
    }

    pub fn from_reader<T: ReadBytesExt + std::io::Seek>(
        read: &mut T,
    ) -> Result<ZipEOCD, ZipReadError> {
        let mut eocd = ZipEOCD::empty();
        let zip_size = read.seek(SeekFrom::End(0))?;
        // comment is 65535 bytes at most
        // from: https://github.com/mvdnes/zip-rs/blob/003440bfe3823a01f11047c42e441999f0554daf/src/spec.rs
        let zip_eocd_left_bound_pos = zip_size
            .checked_sub(
                (u16::MAX as u64)
                    + (std::mem::size_of::<ZipEOCD>() as u64)
                    + (EOCD_MAGIC.len() as u64),
            )
            .unwrap_or(0);
        let mut pos = read.seek(SeekFrom::Start(zip_eocd_left_bound_pos))?;

        // Start searching for candicdates of magick numbers
        let mut eocd_magic_point: usize = 0;
        let mut buf_u8: [u8; 1] = [0];
        while read.read_exact(&mut buf_u8).is_ok() {
            // not magick numbers
            if EOCD_MAGIC[eocd_magic_point] != buf_u8[0] {
                eocd_magic_point = if EOCD_MAGIC[0] == buf_u8[0] {
                    1 as usize
                } else {
                    0 as usize
                };

                pos += 1;
                continue;
            }
            eocd_magic_point += 1;
            // magick numbers found
            if eocd_magic_point >= EOCD_MAGIC.len() {
                if eocd.from_reader_next_to_signature(read)? {
                    return Ok(eocd);
                }
                // not magick numbers
                // Restore pre-check state
                read.seek(SeekFrom::Start(pos))?;
                eocd_magic_point = 0;
            }
            pos += 1;
        }
        return Err(ZipReadError::InvalidZipArchive {
            reason: format!(
                "valid end of central directory signature (PK\\x05\\x06) was not found"
            ),
        });
    }

    /// 分割されたZIPファイルでなければtrue
    pub fn is_single_archive(&self) -> bool {
        return self.eocd_disk_index == 0 && self.n_cd_entries == self.n_cd_entries_in_disk;
    }

    /// ZIP64ならtrue
    pub fn is_zip64(&self) -> bool {
        // Prioritize the ones that are likely to overflow.
        return self.cd_starting_position == u32::MAX
            || self.cd_size == u32::MAX
            || self.n_cd_entries == u16::MAX
            || self.n_cd_entries_in_disk == u16::MAX
            || self.eocd_disk_index == u16::MAX
            || self.cd_start_disk_index == u16::MAX;
    }

    pub fn check_unsupported_zip_type(&self) -> Result<(), ZipReadError> {
        if !self.is_single_archive() {
            return Err(ZipReadError::UnsupportedZipArchive {
                reason: "it is one of splitted arvhives".to_string(),
            });
        }
        if self.is_zip64() {
            return Err(ZipReadError::UnsupportedZipArchive {
                reason: "it is ZIP64 formatted".to_string(),
            });
        }
        return Ok(());
    }
}
