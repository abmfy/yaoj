use chrono::{SecondsFormat, Utc};
use serde::Serializer;

pub mod contests;
pub mod jobs;
pub mod users;

pub mod err;

fn serialize_date_time<S>(dt: &chrono::DateTime<Utc>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    s.serialize_str(dt.to_rfc3339_opts(SecondsFormat::Millis, true).as_str())
}
