use anyhow::Context;
use rocket::http::Status;
use tracing::{error, span, warn, Level};

use crate::{
	attendance::get_attendable_events, db::Database, events::get_relevant_events, member::Member,
	State,
};

use super::SessionID;

pub async fn create_attendance_panel(
	member: &Member,
	db: &impl Database,
) -> anyhow::Result<String> {
	let out = include_str!("components/attendance_panel.min.html");

	let events = db
		.get_events()
		.await
		.context("Failed to get events from database")?
		.collect::<Vec<_>>();
	let events = get_relevant_events(member, events.iter());
	let events = get_attendable_events(events);
	if events.is_empty() {
		return Ok("<h4>No events to attend</h4>".into());
	}

	let current_attendance = db
		.get_current_attendance(&member.id)
		.await
		.context("Failed to get current attendance for member from database")?;

	let mut items = String::new();
	for event in events {
		let (props, button_message) = if let Some(current_attendance) = &current_attendance {
			if current_attendance.event == event.id {
				(" id=\"attending\"", "Leave")
			} else {
				(" style=\"display:none\"", "Attend")
			}
		} else {
			("", "Attend")
		};

		let elem = include_str!("components/attendance_item.min.html");
		let elem = elem.replace("{{props}}", props);
		let elem = elem.replace("{{button-message}}", button_message);
		let elem = elem.replace("{{name}}", &event.name);
		let elem = elem.replace("{{id}}", &event.id);
		items.push_str(&elem);
	}
	let out = out.replace("{{items}}", &items);

	Ok(out)
}

#[rocket::post("/api/attend/<event>")]
pub async fn attend(event: &str, session_id: SessionID<'_>, state: &State) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Attend API");
	let _enter = span.enter();

	let mut lock = state.db.lock().await;

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
		.is_some()
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

#[rocket::post("/api/unattend")]
pub async fn unattend(session_id: SessionID<'_>, state: &State) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Unattend API");
	let _enter = span.enter();

	let mut lock = state.db.lock().await;

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

	if lock
		.get_current_attendance(&member)
		.await
		.map_err(|e| {
			error!("Failed to get attendance from database: {e}");
			Status::InternalServerError
		})?
		.is_none()
	{
		warn!(
			"Attempted to finish attendance while not attending: {}",
			member
		);
		return Err(Status::BadRequest);
	}

	if let Err(e) = lock.finish_attendance(&member).await {
		error!("Failed to record unattendance to database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}
