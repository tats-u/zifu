#[derive(thiserror::Error, Debug)]
pub enum ZipReadError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error("the file seems not to be a valid ZIP archive because: {reason}")]
    InvalidZipArchive { reason: String },
    #[error("this ZIP archive is not supported because: {reason}")]
    UnsupportedZipArchive { reason: String },
}
