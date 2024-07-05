use std::{fmt::Display, ops::Deref};

use anyhow::Context;
use chrono::{DateTime, NaiveDate, NaiveTime, Offset, TimeZone, Utc};
use itertools::Itertools;
use rocket::{
	form::Form,
	http::Status,
	response::{content::RawHtml, Redirect},
	FromForm,
};
use tracing::{error, span, Level};

use crate::{
	auth::Privilege,
	db::Database,
	events::{Event, EventKind, EventUrgency, EventVisibility},
	generate_id,
	member::{count_group_members, MemberGroup, MemberMention},
	render_date,
};
use crate::{events::get_relevant_events, State};

use super::{create_page, OptionalSessionID, PageOrRedirect, SessionID};

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

	let lock = state.db.lock().await;
	let mut relevant_events = get_relevant_events(&member, lock.get_events());
	relevant_events
		.sort_by_cached_key(|x| DateTime::parse_from_rfc2822(&x.date).unwrap_or_default());

	let event_component = include_str!("components/event.min.html");
	let mut events_content = String::with_capacity(relevant_events.len() * event_component.len());
	for event in relevant_events {
		events_content.push_str(&render_event(event, lock.deref()));
	}

	let page = include_str!("pages/calendar.min.html");
	let page = page.replace("{{events}}", &events_content);

	let is_elevated = member.kind.get_privilege() == Privilege::Elevated;
	let new_button = if is_elevated {
		format!(
			"<a href=\"/create_event\">{}</a>",
			include_str!("components/new.min.html")
		)
	} else {
		String::new()
	};
	let page = page.replace("{{add-event}}", &new_button);

	let page = create_page("Calendar", &page);
	Ok(PageOrRedirect::Page(RawHtml(page)))
}

/// Renders an event component
fn render_event(event: &Event, db: &impl Database) -> String {
	let event_component = include_str!("components/event.min.html");

	let date = DateTime::parse_from_rfc2822(&event.date)
		.map(|x| render_date(x))
		.unwrap_or_else(|e| {
			error!("Failed to parse date {}: {}", event.date, e);
			"Invalid date".into()
		});
	let event_component = event_component.replace("{{date}}", &date);
	let event_component = event_component.replace("{{name}}", &event.name);

	let total_invites = event.invites.iter().fold(0, |acc, x| {
		acc + match x {
			MemberMention::Member(_) => 1,
			MemberMention::Group(group) => count_group_members(db.get_members(), group),
		}
	});
	let total_rsvps = event.rsvp.len();
	let event_component = event_component.replace("{{kind}}", &event.kind.to_string());
	let event_component = event_component.replace("{{invites}}", &total_invites.to_string());
	let event_component = event_component.replace("{{going}}", &total_rsvps.to_string());

	event_component
}

#[rocket::get("/create_event?<id>")]
pub async fn create_event(
	id: Option<&str>,
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Create event page");
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

	if member.kind.get_privilege() != Privilege::Elevated {
		error!("Invalid permissions");
		return Ok(redirect);
	}

	let lock = state.db.lock().await;
	let event = if let Some(id) = id {
		// We are editing an existing event
		lock.get_event(id).ok_or_else(|| {
			error!("Event does not exist {}", id);
			Status::Unauthorized
		})?
	} else {
		// We are making a new event
		let id = generate_id();
		Event {
			id,
			name: String::new(),
			date: date_to_js(
				Utc::now()
					.with_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap())
					.unwrap(),
			),
			kind: Default::default(),
			urgency: Default::default(),
			visibility: Default::default(),
			invites: Default::default(),
			rsvp: Default::default(),
		}
	};

	let page = include_str!("pages/create_event.html");
	let page = page.replace("{{id}}", &event.id);
	let page = page.replace("{{name}}", &event.name);
	let page = page.replace("{{date}}", &event.date);
	let page = page.replace("{{kind}}", &serde_json::to_string(&event.kind).unwrap());
	let page = page.replace(
		"{{urgency}}",
		&serde_json::to_string(&event.urgency).unwrap(),
	);
	let page = page.replace(
		"{{visibility}}",
		&serde_json::to_string(&event.visibility).unwrap(),
	);

	// Generate invite checkboxes
	let mut invites_string = String::new();
	let mut available_invites = Vec::with_capacity(6 + event.invites.len());
	for group in [
		MemberGroup::Member,
		MemberGroup::NewMember,
		MemberGroup::PitCrew,
		MemberGroup::Lead,
		MemberGroup::President,
		MemberGroup::Coach,
		MemberGroup::Mentor,
	] {
		available_invites.push((
			format!("@{}", group.to_string()),
			format!(
				"<div class=\"group-invite-label\">{}</div>",
				group.to_plural_string().to_string()
			),
		));
	}
	available_invites.extend(
		lock.get_members()
			.map(|x| {
				let id = x.id.clone();
				let name = lock
					.get_member(&id)
					.map(|x| x.name.clone())
					.unwrap_or_else(|| {
						error!("Failed to get member {}", id);
						id.clone()
					});

				(id, name)
			})
			.sorted_by_key(|x| x.1.clone()),
	);

	for (i, (invite, invite_pretty)) in available_invites.into_iter().enumerate() {
		let invite =
			format!("<div class=\"container invite-checkbox\"><label for=\"{invite}\">{invite_pretty}</label><input type=\"checkbox\" name=\"{invite}\" id=\"invite-checkbox-{i}\" /></div>");
		invites_string.push_str(&invite);
	}
	let page = page.replace("{{invites}}", &invites_string);

	let page = create_page("Create Event", &page);

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

#[rocket::post("/api/create_event", data = "<event>")]
pub async fn create_event_api(
	session_id: SessionID<'_>,
	state: &State,
	event: Form<EventForm>,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Create event API");
	let _enter = span.enter();

	session_id.verify_elevated(state).await?;

	let event = event.into_inner();

	let date = match date_from_js(event.date) {
		Ok(date) => date,
		Err(e) => {
			error!("Failed to parse date: {}", e);
			return Err(Status::InternalServerError);
		}
	};

	let Ok(invites) = serde_json::from_str::<Vec<String>>(&event.invites) else {
		error!("Failed to parse invites");
		return Err(Status::InternalServerError);
	};
	let invites = invites
		.into_iter()
		.map(|x| match x.as_str() {
			"@Member" => MemberMention::Group(MemberGroup::Member),
			"@New Member" => MemberMention::Group(MemberGroup::NewMember),
			"@Pit Crew" => MemberMention::Group(MemberGroup::PitCrew),
			"@Lead" => MemberMention::Group(MemberGroup::Lead),
			"@President" => MemberMention::Group(MemberGroup::President),
			"@Coach" => MemberMention::Group(MemberGroup::Coach),
			"@Mentor" => MemberMention::Group(MemberGroup::Mentor),
			_ => MemberMention::Member(x),
		})
		.collect();

	let mut lock = state.db.lock().await;
	let existing_event = lock.get_event(&event.id);
	let existing_rsvps = existing_event.as_ref().map(|x| x.rsvp.clone());

	let event = Event {
		id: event.id,
		name: event.name,
		date: date.to_rfc2822(),
		kind: event.kind,
		urgency: event.urgency,
		visibility: event.visibility,
		invites,
		rsvp: existing_rsvps.unwrap_or_default(),
	};

	if let Err(e) = lock.create_event(event) {
		error!("Failed to create event: {}", e);
		return Err(Status::InternalServerError);
	}

	Ok(())
}

#[derive(FromForm)]
pub struct EventForm {
	id: String,
	name: String,
	date: String,
	kind: EventKind,
	urgency: EventUrgency,
	visibility: EventVisibility,
	invites: String,
}

/// Formats a date as JS/HTML's version
fn date_to_js<T: TimeZone + Offset>(date: DateTime<T>) -> String
where
	T::Offset: Display,
{
	date.format("%Y-%m-%dT%H:%M").to_string()
}

/// Parses a date from JS/HTML's version
fn date_from_js(date: String) -> anyhow::Result<DateTime<Utc>> {
	let year = date[0..4].parse().context("Failed to parse year")?;
	let month = date[5..7].parse().context("Failed to parse month")?;
	let day = date[8..10].parse().context("Failed to parse day")?;
	// FIXME: Use the actual time zone instead of just assuming US east
	let hour = date[11..13]
		.parse::<u32>()
		.context("Failed to parse hour")?
		+ 4;
	let min = date[14..16].parse().context("Failed to parse minute")?;

	let naive_dt = NaiveDate::from_ymd_opt(year, month, day)
		.context("Failed to create date")?
		.and_hms_opt(hour, min, 0)
		.context("Failed to add time to date")?;
	Ok(naive_dt.and_utc())
}
