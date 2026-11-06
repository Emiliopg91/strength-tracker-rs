use thiserror::Error;

#[derive(Error, Debug)]
pub enum MtpError {
    #[error("Cannot list devices: {0}")]
    ListDevices(#[source] mtp_rs::Error),
    #[error("Cannot open device {0}: {1}")]
    OpenDevice(u64, #[source] mtp_rs::Error),
    #[error("Cannot access storage: {0}")]
    Storage(#[source] mtp_rs::Error),
    #[error("Cannot list files: {0}")]
    ListFiles(#[source] mtp_rs::Error),
    #[error("Cannot find device with serial number {0}")]
    MissingDevice(String),
    #[error("No storage for device with serial number {0}")]
    NoStorageDevice(String),
    #[error("Invalid file name for upload: {0}")]
    InvalidFileName(String),
    #[error("Cannot read data from {0}: {1}")]
    ReadData(String, #[source] std::io::Error),
    #[error("Cannot delete file {0}: {1}")]
    DeleteFile(String, #[source] mtp_rs::Error),
    #[error("Cannot upload file {0}: {1}")]
    UploadFile(String, #[source] mtp_rs::UploadError),
}

pub type Result<T> = std::result::Result<T, MtpError>;
