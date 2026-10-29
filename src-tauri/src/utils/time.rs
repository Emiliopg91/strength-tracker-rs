use chrono::{DateTime, Local, TimeZone};

/// Rounds a UTC timestamp in seconds down to the start of its day in local time.
pub fn local_midnight(ts: i64) -> Option<u32> {
    DateTime::from_timestamp(ts, 0)
        .and_then(|dt| dt.with_timezone(&Local).date_naive().and_hms_opt(0, 0, 0))
        .and_then(|midnight| Local.from_local_datetime(&midnight).earliest())
        .and_then(|dt| u32::try_from(dt.timestamp()).ok())
}
