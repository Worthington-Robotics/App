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
	db::{Database, DatabaseImpl},
	events::{format_minutes, get_season, get_upcoming_events, Event},
	member::Member,
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

/// Stats for attendance
#[derive(Default, Clone, Debug)]
pub struct AttendanceStats {
	pub attended_events: u32,
	pub total_events: u32,
	pub attended_minutes: u32,
	pub total_minutes: u32,
}

impl AttendanceStats {
	/// Format the attended / total ratio
	pub fn format_ratio(&self) -> String {
		format!("{}/{}", self.attended_events, self.total_events)
	}

	/// Format the attended percentage
	pub fn format_percent(&self) -> String {
		let num = if self.total_events == 0 {
			0.0
		} else {
			self.attended_events as f32 / self.total_events as f32 * 100.0
		};
		format!("{:.1}%", num)
	}

	/// Format the average time per event
	pub fn format_average(&self) -> String {
		let num = if self.total_events == 0 {
			0
		} else {
			self.attended_minutes / self.total_events
		};
		format_minutes(num)
	}
}

/// Gets attendance stats for a member for this season and all time
pub fn get_attendance_stats(
	member: &Member,
	db: &impl Database,
) -> (AttendanceStats, AttendanceStats) {
	let mut season = AttendanceStats::default();
	let mut all_time = AttendanceStats::default();

	let now = Utc::now();
	let current_season = get_season(&now);
	let attendances = db.get_attendance(&member.id);
	for event in db.get_events().filter(|x| x.invites_member(member)) {
		let Ok(date) = DateTime::parse_from_rfc2822(&event.date) else {
			error!("Failed to parse date for event {}", event.id);
			continue;
		};
		let date = date.with_timezone(&Utc);

		let Ok(end_date) = event.get_end_date() else {
			error!("Failed to parse end date for event {}", event.id);
			continue;
		};
		let end_date = end_date.with_timezone(&Utc);

		let attendances = attendances
			.iter()
			.filter(|x| x.is_complete() && x.event == event.id);

		let total_minutes = attendances.fold(0, |acc, x| {
			let Ok(start_date) = DateTime::parse_from_rfc2822(&x.start_time) else {
				error!("Failed to parse start date for attendance");
				return acc;
			};

			let Ok(end_date) = DateTime::parse_from_rfc2822(
				x.end_time
					.as_ref()
					.expect("Attendance item should be complete"),
			) else {
				error!("Failed to parse end date for attendance");
				return acc;
			};

			let delta = (end_date - start_date).num_minutes() as u32;
			acc + delta
		});

		let event_duration = (end_date - date).num_minutes() as u32;
		let attended_percentage = total_minutes as f32 / event_duration as f32;
		// Only count events where the member stayed for either an hour or most of the event
		let attended_event = attended_percentage >= 0.8 || total_minutes > 60;

		// Add to total stats
		all_time.total_events += 1;
		if attended_event {
			all_time.attended_events += 1;
		}
		all_time.total_minutes += event_duration;
		all_time.attended_minutes += total_minutes;

		// Add to season stats
		let event_season = get_season(&date);
		if event_season == current_season {
			season.total_events += 1;
			if attended_event {
				season.attended_events += 1;
			}
			season.total_minutes += event_duration;
			season.attended_minutes += total_minutes;
		}
	}

	(season, all_time)
}

/// Fairing for managing attendance
pub struct AttendanceFairing {
	db: Arc<Mutex<DatabaseImpl>>,
}

impl AttendanceFairing {
	pub fn new(db: Arc<Mutex<DatabaseImpl>>) -> Self {
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
