use chrono::{DateTime, Utc};

pub mod db;
pub mod telemetry;

pub type Time = DateTime<Utc>;
