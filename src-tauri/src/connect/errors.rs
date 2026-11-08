use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConnectError {
    #[error("Error on OAuth2 login: {0}")]
    Login(#[source] crate::connect::client::GarminError),
    #[error("Login required")]
    LoginRequired(),
    #[error("Login Cancelled by user")]
    LoginCancelledByUser(),
    #[error("OAuth error: {0}")]
    OAuthRefresh(#[source] crate::connect::client::GarminError),
    #[error("Keyring error: {0}")]
    Keyring(#[from] keyring::Error),
    #[error("Token serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(u32),
    #[error("Error fetching activities: {0}")]
    FetchActivities(#[source] crate::connect::client::GarminError),
    #[error("Error fetching weight: {0}")]
    FetchWeight(#[source] crate::connect::client::GarminError),
    #[error("Error downloading activity '{0}': {1}")]
    DownloadActivity(u64, #[source] crate::connect::client::GarminError),
    #[error("Error unzipping activity '{0}': {1}")]
    UnzipActivity(u64, #[source] zip::result::ZipError),
    #[error("Error extracting activity '{0}': {1}")]
    ExtractActivity(u64, #[source] std::io::Error),
    #[error("Error joining extraction task for activity '{0}': {1}")]
    ExtractTask(u64, #[source] tokio::task::JoinError),
    #[error("Error fetching devices: {0}")]
    FetchDevices(#[source] crate::connect::client::GarminError),
    #[error("Error fetching calendar: {0}")]
    FetchCalendar(#[source] crate::connect::client::GarminError),
    #[error("Error fetching workouts: {0}")]
    FetchWorkouts(#[source] crate::connect::client::GarminError),
    #[error("Error creating workout: {0}")]
    CreateWorkout(#[source] crate::connect::client::GarminError),
    #[error("Error updating workout '{0}': {1}")]
    UpdateWorkout(u64, #[source] crate::connect::client::GarminError),
    #[error("Error sending workout '{0}' to device '{1}': {2}")]
    SendWorkout(u64, u64, #[source] crate::connect::client::GarminError),
}

pub type Result<T> = std::result::Result<T, ConnectError>;
