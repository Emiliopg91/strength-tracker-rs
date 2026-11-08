use serde::{Deserialize, Serialize};

/// Input the backend asks the user for through a frontend dialog.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptKind {
    /// Garmin Connect email and password. Answered with `[email, password]`.
    ConnectCredentials,
    /// Garmin Connect MFA verification code. Answered with `[code]`.
    ConnectMfa,
    /// Password to run the udev rules installation through sudo. Answered with `[password]`.
    SudoPassword,
}

/// Payload of the `prompt_requested` event. The frontend answers it through `answer_prompt` with
/// the same `id`.
#[derive(Serialize, Clone, Debug)]
pub struct PromptRequest {
    pub id: u32,
    pub kind: PromptKind,
    /// Value to prefill the first field with (e.g. the last used email).
    pub prefill: Option<String>,
    /// Kind-specific detail: the error of the previous attempt for `ConnectCredentials` and
    /// `SudoPassword`, the MFA method for `ConnectMfa`.
    pub detail: Option<String>,
}
