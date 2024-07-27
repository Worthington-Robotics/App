use std::{sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use rocket::{
	fairing::{Fairing, Info, Kind},
	tokio::sync::Mutex,
	Orbit, Rocket,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{
	db::json::JSONDatabase,
	db::Database,
	events::{get_upcoming_events, Event},
};

#[derive(Serialize, Deserialize, Clone)]
pub struct AttendanceEntry {
	/// A DateTime
	pub start_time: String,
	/// A DateTime
	pub end_time: Option<String>,
	pub event: String,
}

impl AttendanceEntry {
	/// Check if this entry has been completed (an end time has been set)
	pub fn is_complete(&self) -> bool {
		self.end_time.is_some()
	}
}

/// Get all of the events that are able to be attended
pub fn get_attendable_events<'a>(events: Vec<&'a Event>) -> Vec<&'a Event> {
	if events.is_empty() {
		return events;
	}

	let events = get_upcoming_events(events);

	/// Threshold for how far in the future events can be before they are considered unattendable, in minutes
	const TIME_THRESHOLD: i64 = 30;

	let now = Utc::now();
	events
		.into_iter()
		.filter(|x| {
			let date = DateTime::parse_from_rfc2822(&x.date);
			if let Ok(date) = date {
				let date = date.with_timezone(&Utc);
				let diff = date.timestamp() - now.timestamp();
				if diff > TIME_THRESHOLD * 60 {
					return false;
				}
			}

			true
		})
		.collect()
}

/// Fairing for managing attendance
pub struct AttendanceFairing {
	db: Arc<Mutex<JSONDatabase>>,
}

impl AttendanceFairing {
	pub fn new(db: Arc<Mutex<JSONDatabase>>) -> Self {
		Self { db }
	}
}

#[async_trait::async_trait]
impl Fairing for AttendanceFairing {
	fn info(&self) -> Info {
		Info {
			name: "Attendance",
			kind: Kind::Liftoff,
		}
	}

	async fn on_liftoff(&self, _: &Rocket<Orbit>) {
		// Periodically check for attendances that have been finished by events ending
		let db = self.db.clone();
		rocket::tokio::task::spawn(async move {
			loop {
				rocket::tokio::time::sleep(Duration::from_secs(60)).await;
				let mut lock = db.lock().await;
				let now = Utc::now();
				let members: Vec<_> = lock.get_members().map(|x| x.id.clone()).collect();
				for member in members {
					if let Some(current_attendance) = lock.get_current_attendance(&member) {
						let Some(event) = lock.get_event(&current_attendance.event) else {
							error!("Failed to get event from attendance record");
							continue;
						};
						let Ok(end_date) = event.get_end_date() else {
							error!("Failed to parse date");
							continue;
						};
						if now > end_date.with_timezone(&Utc) {
							if let Err(e) = lock.finish_attendance(&member) {
								error!("Failed to finish attendance for member {member}: {e}");
							}
						}
					}
				}
			}
		});
	}
}
