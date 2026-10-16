use rusqlite_orm::Entity;
use serde::{Deserialize, Serialize};

#[derive(Entity, Clone, Deserialize, Serialize)]
#[entity("additional_data")]
#[primary_key(session)]
pub struct AdditionalData {
    pub session: u32,
    pub heart_rates: Option<Vec<u8>>,
    pub coordinates: Option<Vec<u8>>,
    pub speeds: Option<Vec<u8>>,
    pub altitudes: Option<Vec<u8>>,
    pub distance: Option<f64>,
    pub notes: Option<String>,
    pub max_hr: Option<u8>,
}

impl AdditionalData {
    pub const INVALID_HEAR_RATE: u8 = u8::MAX;
    pub const INVALID_POSITION: i32 = i32::MAX;
    pub const INVALID_SPEED: f64 = -1_f64;
    pub const INVALID_ALTITUDE: f64 = f64::MAX;

    pub const SEMICIRCLE_TO_DEGREES: f64 = 180.0 / (2_i64.pow(31) as f64);

    pub fn new(session: u32) -> Self {
        Self {
            session,
            altitudes: None,
            coordinates: None,
            distance: None,
            heart_rates: None,
            notes: None,
            speeds: None,
            max_hr: None,
        }
    }

    /// Unpacks the raw byte blob into (lat, lon) pairs in semicircles.
    pub fn get_coordinates_semicircle(&self) -> Option<Vec<Option<(i32, i32)>>> {
        self.coordinates.as_ref().map(|records| {
            records
                .as_chunks::<8>()
                .0
                .iter()
                .map(|chunk| {
                    let lat = i32::from_be_bytes(chunk[0..4].try_into().unwrap());
                    let lon = i32::from_be_bytes(chunk[4..8].try_into().unwrap());
                    if lat != Self::INVALID_POSITION && lon != Self::INVALID_POSITION {
                        Some((lat, lon))
                    } else {
                        None
                    }
                })
                .collect()
        })
    }
    /// Unpacks the raw byte blob into (lat, lon) pairs converted to degrees.
    pub fn get_coordinates_degrees(&self) -> Option<Vec<Option<(f64, f64)>>> {
        self.get_coordinates_semicircle().map(|semicircles| {
            semicircles
                .into_iter()
                .map(|p| {
                    p.map(|p| {
                        (
                            p.0 as f64 * Self::SEMICIRCLE_TO_DEGREES,
                            p.1 as f64 * Self::SEMICIRCLE_TO_DEGREES,
                        )
                    })
                })
                .collect()
        })
    }
    /// Build blob from semicircles coordinates Vec
    pub fn build_coordinates_blob(values: &[(i32, i32)]) -> Vec<u8> {
        let mut coords = Vec::new();

        for (latitude, longitude) in values {
            coords.extend_from_slice(&latitude.to_be_bytes());
            coords.extend_from_slice(&longitude.to_be_bytes());
        }

        coords
    }

    /// Unpacks the altitudes Blob into Vec
    pub fn get_altitudes(&self) -> Option<Vec<Option<f64>>> {
        self.altitudes.as_ref().map(|records| {
            records
                .as_chunks::<8>()
                .0
                .iter()
                .map(|chunk| {
                    let val = f64::from_be_bytes(*chunk);
                    if val != Self::INVALID_ALTITUDE {
                        Some(val)
                    } else {
                        None
                    }
                })
                .collect()
        })
    }
    /// Build blob from altitudes Vec
    pub fn build_altitudes_blob(value: &[f64]) -> Vec<u8> {
        let mut records = Vec::new();

        value.iter().for_each(|speed| {
            records.extend_from_slice(&(*speed).to_be_bytes());
        });

        records
    }

    /// Unpacks the speeds Blob into Vec
    pub fn get_speeds(&self) -> Option<Vec<Option<f64>>> {
        self.speeds.as_ref().map(|records| {
            records
                .as_chunks::<8>()
                .0
                .iter()
                .map(|chunk| {
                    let val = f64::from_be_bytes(*chunk);
                    if val != Self::INVALID_SPEED {
                        Some(val)
                    } else {
                        None
                    }
                })
                .collect()
        })
    }
    /// Build blob from speeds Vec
    pub fn build_speeds_blob(value: &[f64]) -> Vec<u8> {
        let mut records = Vec::new();

        value.iter().for_each(|speed| {
            records.extend_from_slice(&(*speed).to_be_bytes());
        });

        records
    }

    /// Unpacks the heart rate Blob into Vec
    pub fn get_heart_rates(&self) -> Option<Vec<Option<u8>>> {
        self.heart_rates.as_ref().map(|records| {
            records
                .iter()
                .copied()
                .map(|val| {
                    if val < Self::INVALID_HEAR_RATE {
                        Some(val)
                    } else {
                        None
                    }
                })
                .collect()
        })
    }
}
