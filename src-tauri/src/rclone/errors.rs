use std::process::ExitStatus;

use curl_rest::StatusCode;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum RCloneError {
    #[error("Error on authorize command: {0}")]
    Authorization(#[source] std::io::Error),
    #[error("Bad authorization token")]
    BadToken(),
    #[error("Bad response status: {0:?}")]
    BadResponseStatus(StatusCode),
    #[error("Bad exit status from command: {0:?}, stderr: {1}")]
    BadExitStatus(ExitStatus, String),
    #[error("Error running command: {0}")]
    CommandError(#[source] std::io::Error),
    #[error("Bad JSON format: {0}")]
    BadJsonFormat(#[source] serde_json::error::Error),
    #[error("Error getting drives: {0:?}")]
    ErrorGettingDrives(#[source] curl_rest::Error),
    #[error("Error waiting for task: {0:?}")]
    TaskError(#[source] tokio::task::JoinError),
    #[error("No drive found")]
    NoDriveFound(),
    #[error("Error reading config file: {0}")]
    ReadConfig(#[source] std::io::Error),
    #[error("Missing or invalid field '{0}' in JSON response")]
    MissingJsonField(&'static str),
}

pub type Result<T> = std::result::Result<T, RCloneError>;
