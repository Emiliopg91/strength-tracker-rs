use thiserror::Error;

#[derive(Error, Debug)]
pub enum UdevError {
    #[error("Error writting udev rules: {0}")]
    Write(#[source] std::io::Error),
    #[error("Error reloading udev rules: {0}")]
    Reload(#[source] std::io::Error),
    #[error("Error triggering udev rules: {0}")]
    Trigger(#[source] std::io::Error),
    #[error("Incorrect password")]
    WrongPassword,
}

pub type Result<T> = std::result::Result<T, UdevError>;
