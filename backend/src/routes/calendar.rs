use chrono::DateTime;
use rocket::{
	http::Status,
	response::{content::RawHtml, Redirect},
};
use tracing::{error, span, Level};

use crate::{auth::Privilege, db::Database};
use crate::{events::get_relevant_events, State};

use super::{create_page, OptionalSessionID, PageOrRedirect};

#[rocket::get("/calendar")]
pub async fn calendar(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Calendar");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.id else {
		return Ok(redirect);
	};

	let Some(requesting_member_id) = ({
		let lock = state.session_manager.lock().await;
		lock.get(session_id).map(|x| x.member.clone())
	}) else {
		error!("Unknown session ID {}", session_id);
		return Ok(redirect);
	};

	let Some(member) = ({
		let lock = state.db.lock().await;
		lock.get_member(&requesting_member_id)
	}) else {
		error!("Unknown requesting member ID {}", requesting_member_id);
		return Ok(redirect);
	};

	let is_elevated = member.kind.get_privilege() == Privilege::Elevated;

	let lock = state.db.lock().await;
	let mut relevant_events = get_relevant_events(&member, lock.get_events());
	relevant_events
		.sort_by_cached_key(|x| DateTime::parse_from_rfc2822(&x.date).unwrap_or_default());

	let event_component = include_str!("components/event.html");
	let mut events_content = String::with_capacity(relevant_events.len() * event_component.len());
	for event in relevant_events {
		let date = DateTime::parse_from_rfc2822(&event.date)
			.map(|x| x.to_string())
			.unwrap_or_else(|e| {
				error!("Failed to parse date {}: {}", event.date, e);
				"Invalid date".into()
			});
		let event_component = event_component.replace("{date}", &date);
		let event_component = event_component.replace("{name}", &event.name);
		events_content.push_str(&event_component);
	}

	let page = include_str!("pages/calendar.html");
	let page = page.replace("{events}", &events_content);
	let page = create_page("Calendar", &page);
	Ok(PageOrRedirect::Page(RawHtml(page)))
}
