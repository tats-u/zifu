/// Extended `std::io::Error` for ZIP archive
#[derive(thiserror::Error, Debug)]
pub enum ZipReadError {
    /// See `std::io::Error`
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    /// An error due to invalid ZIP arvhie
    #[error("the file seems not to be a valid ZIP archive because: {reason}")]
    InvalidZipArchive { reason: String },
    /// An error due to unsupported ZIP archive in this software
    #[error("this ZIP archive is not supported because: {reason}")]
    UnsupportedZipArchive { reason: String },
}
