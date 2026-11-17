use thiserror::Error;

#[derive(Error, Debug)]
pub enum WriteFitFileError {
    #[error("Error while encoding FIT file: {0}")]
    Encoding(#[source] Box<dyn std::error::Error + Send + Sync>),
    #[error("Error while writing file {0}: {1}")]
    FileWriting(String, #[source] std::io::Error),
}

pub type Result<T> = std::result::Result<T, WriteFitFileError>;
