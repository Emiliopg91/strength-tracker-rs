//! Garmin Connect client, vendored from `garmin-cli` 2.2.0 and trimmed down to
//! SSO login, token refresh, activity listing and activity details.
//!
//! Upstream: <https://github.com/vicentereig/garmin-cli>
//! Copyright (c) Vicente Reig Rincon de Arellano. Licensed under the MIT license.

pub mod api;
pub mod error;
mod http;
pub mod models;
pub mod oauth1;
pub mod sso;
pub mod tokens;

pub use api::GarminClient;
pub use error::{GarminError, Result};
pub use models::{ActivityDetails, ActivitySummary, ActivityType, WeightEntry, WeightRange};
pub use sso::SsoClient;
pub use tokens::{OAuth1Token, OAuth2Token};

/// International (global) Garmin domain.
pub const GLOBAL_DOMAIN: &str = "garmin.com";

/// Mainland China Garmin domain.
pub const CHINA_DOMAIN: &str = "garmin.cn";
