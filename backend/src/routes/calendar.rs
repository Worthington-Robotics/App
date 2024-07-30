use std::{fmt::Display, ops::Deref, str::FromStr};

use anyhow::Context;
use chrono::{DateTime, Duration, NaiveDate, NaiveTime, Offset, TimeZone, Utc};
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
	member::{count_group_members, Member, MemberGroup, MemberMention},
	render_date,
	util::{get_days_from_month, ToDropdown},
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
		lock.get_member(&requesting_member_id).await.map_err(|e| {
			error!("Failed to get member from database: {e}");
			Status::InternalServerError
		})?
	}) else {
		error!("Unknown requesting member ID {}", requesting_member_id);
		return Ok(redirect);
	};

	let is_elevated = member.is_elevated();

	let lock = state.db.lock().await;
	let events = lock
		.get_events()
		.await
		.map_err(|e| {
			error!("Failed to get events from database: {e}");
			Status::InternalServerError
		})?
		.into_iter()
		.collect::<Vec<_>>();
	let mut relevant_events = get_relevant_events(&member, events.iter());
	relevant_events
		.sort_by_cached_key(|x| DateTime::parse_from_rfc2822(&x.date).unwrap_or_default());

	let event_component = include_str!("components/event.min.html");
	let mut events_content = String::with_capacity(relevant_events.len() * event_component.len());
	let now = Utc::now();
	for event in relevant_events {
		events_content.push_str(&render_event(event, lock.deref(), &member, &now).await?);
	}

	let page = include_str!("pages/events/calendar.min.html");
	let page = page.replace("{{events}}", &events_content);

	let new_button = if is_elevated {
		format!(
			"<a href=\"/create_event\">{}</a>",
			include_str!("components/ui/new.min.html")
		)
	} else {
		String::new()
	};
	let page = page.replace("{{add-event}}", &new_button);

	let page = create_page("Calendar", &page);
	Ok(PageOrRedirect::Page(RawHtml(page)))
}

/// Renders an event component
async fn render_event(
	event: &Event,
	db: &impl Database,
	member: &Member,
	now: &DateTime<Utc>,
) -> Result<String, Status> {
	let event_component = include_str!("components/event.min.html");
	let event_component = event_component.replace("{{id}}", &event.id);

	let date = DateTime::parse_from_rfc2822(&event.date)
		.map(|x| render_date(x))
		.unwrap_or_else(|e| {
			error!("Failed to parse date {}: {}", event.date, e);
			"Invalid date".into()
		});
	let event_component = event_component.replace("{{date}}", &date);
	let event_component = event_component.replace("{{name}}", &event.name);

	let group_members = db
		.get_members()
		.await
		.map_err(|e| {
			error!("Failed to get members from database: {e}");
			Status::InternalServerError
		})?
		.collect::<Vec<_>>();
	let total_invites = event.invites.iter().fold(0, |acc, x| {
		acc + match x {
			MemberMention::Member(_) => 1,
			MemberMention::Group(group) => count_group_members(group_members.iter(), group),
		}
	});
	let total_rsvps = event.rsvp.len();
	let event_component = event_component.replace("{{kind}}", &event.kind.to_string());
	let event_component = event_component.replace("{{invites}}", &total_invites.to_string());
	let event_component = event_component.replace("{{going}}", &total_rsvps.to_string());

	let edit = if member.is_elevated() {
		include_str!("components/ui/edit.min.html")
	} else {
		""
	};
	let event_component = event_component.replace("{{edit}}", edit);

	// Add the not-upcoming class and display style to relevant events so they can be filtered out in the UI
	let (upcoming_class, upcoming_props) = if event.is_upcoming(now) {
		("", "")
	} else {
		(" not-upcoming", " style=\"display:none\"")
	};
	let event_component = event_component.replace("{{upcoming-class}}", upcoming_class);
	let event_component = event_component.replace("{{upcoming-props}}", upcoming_props);

	Ok(event_component)
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
		lock.get_member(&requesting_member_id).await.map_err(|e| {
			error!("Failed to get member from database: {e}");
			Status::InternalServerError
		})?
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
		lock.get_event(id)
			.await
			.map_err(|e| {
				error!("Failed to get event from database: {e}");
				Status::InternalServerError
			})?
			.ok_or_else(|| {
				error!("Event does not exist: {}", id);
				Status::InternalServerError
			})?
	} else {
		// We are making a new event
		let id = generate_id();
		let date = Utc::now()
			.with_time(NaiveTime::from_hms_opt(12, 0, 0).unwrap())
			.unwrap()
			.to_rfc2822();
		Event {
			id,
			name: String::new(),
			date,
			end_date: None,
			kind: Default::default(),
			urgency: Default::default(),
			visibility: Default::default(),
			invites: Default::default(),
			rsvp: Default::default(),
		}
	};

	let page = include_str!("pages/events/create_event.min.html");
	let page = page.replace("{{id}}", &event.id);
	let page = page.replace("{{name}}", &event.name);

	let date = DateTime::parse_from_rfc2822(&event.date).unwrap_or_else(|e| {
		error!("Failed to parse date: {}", e);
		Default::default()
	});
	let date_str = date_to_js(date);
	let page = page.replace("{{date}}", &date_str);
	let end_date = if let Some(end_date) = &event.end_date {
		date_to_js(DateTime::parse_from_rfc2822(end_date).unwrap_or_else(|e| {
			error!("Failed to parse date: {}", e);
			Default::default()
		}))
	} else {
		// Use the same end date as start date so that they can easily leave it out
		date_str
	};
	let page = page.replace("{{end-date}}", &end_date);
	let (end_date_checked, end_date_enable) = if event.end_date.is_some() {
		(" checked", "")
	} else {
		("", " disabled")
	};
	let page = page.replace("{{end-date-checked}}", end_date_checked);
	let page = page.replace("{{end-date-enable}}", end_date_enable);

	// Create dropdown options
	let page = page.replace(
		"{{kind-options}}",
		&EventKind::create_options(Some(&event.kind)),
	);
	let page = page.replace(
		"{{urgency-options}}",
		&EventUrgency::create_options(Some(&event.urgency)),
	);
	let page = page.replace(
		"{{visibility-options}}",
		&EventVisibility::create_options(Some(&event.visibility)),
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
		let checked = event.invites.contains(&MemberMention::Group(group));
		available_invites.push((
			format!("@{}", group.to_string()),
			format!(
				"<div class=\"group-invite-label\">{}</div>",
				group.to_plural_string().to_string()
			),
			checked,
		));
	}
	for member in lock
		.get_members()
		.await
		.map_err(|e| {
			error!("Failed to get members from database: {e}");
			Status::InternalServerError
		})?
		.sorted_by_key(|x| x.name.clone())
	{
		let id = member.id.clone();
		let name = lock
			.get_member(&id)
			.await
			.ok()
			.and_then(|x| x.map(|x| x.name.clone()))
			.unwrap_or_else(|| {
				error!("Failed to get member {}", id);
				id.clone()
			});
		let checked = event.invites.contains(&MemberMention::Member(id.clone()));
		available_invites.push((id, name, checked));
	}

	for (i, (invite, invite_pretty, is_checked)) in available_invites.into_iter().enumerate() {
		let label = format!("<label for=\"{invite}\">{invite_pretty}</label>");
		let checked_string = if is_checked { " checked" } else { "" };
		let checkbox = format!("<input type=\"checkbox\" name=\"{invite}\" id=\"invite-checkbox-{i}\" {checked_string} />");

		let invite = format!("<div class=\"cont invite-checkbox\">{label}{checkbox}</div>");

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

	let date = match date_from_js(event.date.clone()) {
		Ok(date) => date,
		Err(e) => {
			error!("Failed to parse date {}: {}", event.date, e);
			return Err(Status::BadRequest);
		}
	};

	let end_date = match event.end_date {
		Some(end_date) => {
			let end_date = match date_from_js(end_date) {
				Ok(date) => date,
				Err(e) => {
					error!("Failed to parse date {}: {}", event.date, e);
					return Err(Status::BadRequest);
				}
			};

			// If the end date is less than the start date then we have to reject
			if end_date < date {
				error!("End date {end_date} was less than start date {date}");
				return Err(Status::BadRequest);
			}

			Some(end_date.to_rfc2822())
		}
		None => None,
	};

	let Ok(invites) = serde_json::from_str::<Vec<String>>(&event.invites) else {
		error!("Failed to parse invites");
		return Err(Status::InternalServerError);
	};
	let invites = invites
		.into_iter()
		.map(|x| MemberMention::from_str(&x).unwrap())
		.collect();

	let mut lock = state.db.lock().await;
	let existing_event = lock.get_event(&event.id).await.map_err(|e| {
		error!("Failed to get event from database: {e}");
		Status::InternalServerError
	})?;
	let existing_rsvps = existing_event.as_ref().map(|x| x.rsvp.clone());

	let event = Event {
		id: event.id,
		name: event.name,
		date: date.to_rfc2822(),
		end_date,
		kind: event.kind,
		urgency: event.urgency,
		visibility: event.visibility,
		invites,
		rsvp: existing_rsvps.unwrap_or_default(),
	};

	if let Err(e) = lock.create_event(event).await {
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
	end_date: Option<String>,
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
	// FIXME: Use the actual time zone instead of just assuming US East
	let date = date - Duration::hours(4);
	date.format("%Y-%m-%dT%H:%M").to_string()
}

/// Parses a date from JS/HTML's version
fn date_from_js(date: String) -> anyhow::Result<DateTime<Utc>> {
	let year = date[0..4].parse().context("Failed to parse year")?;
	let mut month = date[5..7].parse().context("Failed to parse month")?;
	let mut day = date[8..10].parse().context("Failed to parse day")?;
	// FIXME: Use the actual time zone instead of just assuming US East
	let mut hour = date[11..13]
		.parse::<u32>()
		.context("Failed to parse hour")?
		+ 4;
	// Chrono doesn't accept overflows, so we move the hours into days instead
	if hour >= 24 {
		day += hour / 24;
		hour %= 24;
	}
	let min = date[14..16].parse().context("Failed to parse minute")?;

	// If the date fails, then we know that we overflowed the day from wrapping the hour. Try wrapping the day now.
	let naive_date = NaiveDate::from_ymd_opt(year, month, day);
	let naive_date = match naive_date {
		Some(date) => Some(date),
		None => {
			let days_in_month = get_days_from_month(year, month) as u32;
			month += day / days_in_month;
			day %= days_in_month;
			NaiveDate::from_ymd_opt(year, month, day)
		}
	}
	.context("Failed to create date")?;

	let naive_dt = naive_date
		.and_hms_opt(hour, min, 0)
		.context("Failed to add time to date")?;
	Ok(naive_dt.and_utc())
}
