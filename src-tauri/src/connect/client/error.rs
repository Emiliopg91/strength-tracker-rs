use thiserror::Error;

#[derive(Error, Debug)]
pub enum GarminError {
    #[error("Authentication error: {0}")]
    Authentication(String),

    #[error("Authentication required")]
    NotAuthenticated,

    #[error("MFA required")]
    MfaRequired,

    #[error("Rate limited. Please wait before retrying.")]
    RateLimited,

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("API error {status}: {message}")]
    Api { status: u16, message: String },

    #[error("HTTP error: {0}")]
    Http(#[from] curl_rest::Error),

    #[error("Error waiting for HTTP task: {0}")]
    Task(#[from] tokio::task::JoinError),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, GarminError>;

impl GarminError {
    pub fn auth(msg: impl Into<String>) -> Self {
        Self::Authentication(msg.into())
    }

    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn invalid_response(msg: impl Into<String>) -> Self {
        Self::InvalidResponse(msg.into())
    }
}
