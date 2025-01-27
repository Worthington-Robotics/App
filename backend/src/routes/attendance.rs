use anyhow::Context;
use chrono::{DateTime, Utc};
use rocket::http::Status;
use tracing::{error, span, warn, Level};

use crate::{
	attendance::get_attendable_events,
	db::Database,
	events::{get_relevant_events, Event},
	member::Member,
	State,
};

use super::SessionID;

pub async fn create_attendance_panel(
	member: &Member,
	db: &impl Database,
) -> anyhow::Result<String> {
	let out = include_str!("components/attendance_panel.min.html");

	let all_events = db
		.get_events()
		.await
		.context("Failed to get events from database")?
		.collect::<Vec<_>>();
	let mut relevant_events = get_relevant_events(member, all_events.iter());
	relevant_events.sort_by_key(|x| x.name.clone());
	let attendable_events = get_attendable_events(relevant_events.clone());

	// Add events that are relevant but are only for RSVP
	let mut events = Vec::with_capacity(attendable_events.len());
	events.extend(attendable_events.into_iter().map(|event| AttendanceEvent {
		event,
		is_rsvp: false,
	}));
	let now = Utc::now();
	for event in relevant_events {
		if !event.is_attendable(&now) {
			let Ok(date) = DateTime::parse_from_rfc2822(&event.date) else {
				continue;
			};
			let diff = date.to_utc() - now;
			if diff.abs().num_hours() <= 36 {
				events.push(AttendanceEvent {
					event,
					is_rsvp: true,
				});
			}
		}
	}

	if events.is_empty() {
		return Ok("<h4>No events to attend</h4>".into());
	}

	// Single event in the list on the attendance panel. Some items are only for RSVP.
	struct AttendanceEvent<'a> {
		is_rsvp: bool,
		event: &'a Event,
	}

	let current_attendance: Vec<_> = db
		.get_current_attendance(&member.id)
		.await
		.context("Failed to get current attendance for member from database")?
		.collect();

	let mut items = String::new();
	for event in events {
		let is_attending = current_attendance.iter().any(|x| x.event == event.event.id);
		let (action, button_message) = if event.is_rsvp {
			if event.event.rsvp.contains(&member.id) {
				("unrsvp", "Remove RSVP")
			} else {
				("rsvp", "RSVP")
			}
		} else {
			if is_attending {
				("unattend", "Leave")
			} else {
				("attend", "Attend")
			}
		};

		let elem = include_str!("components/attendance_item.min.html");
		let elem = elem.replace("{{action}}", action);
		let elem = elem.replace("{{button-message}}", button_message);
		let elem = elem.replace("{{name}}", &event.event.name);
		let elem = elem.replace("{{id}}", &event.event.id);
		items.push_str(&elem);
	}
	let out = out.replace("{{items}}", &items);

	Ok(out)
}

#[rocket::post("/api/attend/<event>")]
pub async fn attend(event: &str, session_id: SessionID<'_>, state: &State) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Attend API");
	let _enter = span.enter();

	let mut lock = state.db.write().await;

	let Some(member) = state
		.session_manager
		.lock()
		.await
		.get(session_id.id)
		.map(|x| x.member.clone())
	else {
		error!("Unknown session ID {}", session_id.id);
		return Err(Status::Unauthorized);
	};

	if !lock.member_exists(&member).await.map_err(|e| {
		error!("Failed to get member from database: {e}");
		Status::InternalServerError
	})? {
		error!("Member {member} does not exist");
	}

	if !lock.event_exists(event).await.map_err(|e| {
		error!("Failed to get event from database: {e}");
		Status::InternalServerError
	})? {
		error!("Event {event} does not exist");
	}

	if lock
		.get_current_attendance(&member)
		.await
		.map_err(|e| {
			error!("Failed to get attendance from database: {e}");
			Status::InternalServerError
		})?
		.any(|x| x.event == event)
	{
		warn!(
			"Attempted to mark new attendance while already attending: {}",
			member
		);
		return Err(Status::BadRequest);
	}

	if let Err(e) = lock.record_attendance(&member, event).await {
		error!("Failed to record attendance to database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

#[rocket::post("/api/unattend/<event>")]
pub async fn unattend(event: &str, session_id: SessionID<'_>, state: &State) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Unattend API");
	let _enter = span.enter();

	let mut lock = state.db.write().await;

	let Some(member) = state
		.session_manager
		.lock()
		.await
		.get(session_id.id)
		.map(|x| x.member.clone())
	else {
		error!("Unknown session ID {}", session_id.id);
		return Err(Status::Unauthorized);
	};

	if !lock.member_exists(&member).await.map_err(|e| {
		error!("Failed to get member from database: {e}");
		Status::InternalServerError
	})? {
		error!("Member {member} does not exist");
	}

	if !lock
		.get_current_attendance(&member)
		.await
		.map_err(|e| {
			error!("Failed to get attendance from database: {e}");
			Status::InternalServerError
		})?
		.any(|x| x.event == event)
	{
		warn!(
			"Attempted to finish attendance while not attending: {}",
			member
		);
		return Err(Status::BadRequest);
	}

	if let Err(e) = lock.finish_attendance(&member, event).await {
		error!("Failed to record unattendance to database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}
