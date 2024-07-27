use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct AttendanceEntry {
	pub start_time: DateTime<Utc>,
	pub end_time: Option<DateTime<Utc>>,
	pub event: String,
}
