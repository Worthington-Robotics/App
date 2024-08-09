use std::collections::HashSet;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use itertools::Itertools;
use rocket::form::{Form, FromForm};
use rocket::http::Status;
use rocket::response::{content::RawHtml, Redirect};
use strum::IntoEnumIterator;
use tracing::{error, span, Level};

use crate::announcements::Announcement;
use crate::db::Database;
use crate::member::{MemberGroup, MemberMention};
use crate::routes::SessionID;
use crate::{generate_id, render_date};
use crate::{routes::OptionalSessionID, State};

use super::{create_page, PageOrRedirect};

#[rocket::get("/inbox")]
pub async fn inbox(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Inbox");
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

	let page = include_str!("pages/announcements/inbox.min.html");

	let lock = state.db.lock().await;
	let announcements = lock
		.get_announcements()
		.await
		.map_err(|e| {
			error!("Failed to get announcements from database: {e}");
			Status::InternalServerError
		})?
		// Sort so that the newest announcements show up at the top
		.sorted_by_cached_key(|x| DateTime::parse_from_rfc2822(&x.date).unwrap_or_default())
		.rev()
		.collect::<Vec<_>>();
	let mut announcements_string = String::with_capacity(
		include_str!("components/announcement.min.html").len() * announcements.len(),
	);

	for announcement in announcements {
		if !announcement.can_member_see(&member) {
			continue;
		}
		announcements_string.push_str(&render_announcement(announcement));
	}
	let page = page.replace("{{announcements}}", &announcements_string);

	let add_button = if member.is_elevated() {
		format!(
			"<a href=\"/create_announcement\">{}</a>",
			include_str!("components/ui/new.min.html")
		)
	} else {
		String::new()
	};

	let page = page.replace("{{add}}", &add_button);

	let page = create_page("Inbox", &page);

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

fn render_announcement(announcement: Announcement) -> String {
	let component = include_str!("components/announcement.min.html");
	let out = component.replace("{{id}}", &announcement.id);
	let date = DateTime::parse_from_rfc2822(&announcement.date)
		.map(render_date)
		.unwrap_or("Invalid date".into());
	let out = out.replace("{{date}}", &date);
	let out = out.replace("{{title}}", &announcement.title);
	let body = announcement
		.body
		.map(|x| {
			// Cut off the end of the body if it is too long
			if x.len() > 35 {
				format!("{}...", &x[0..35])
			} else {
				x
			}
		})
		.unwrap_or_default();
	let out = out.replace("{{body}}", &body);

	out
}

#[rocket::post("/api/create_announcement", data = "<announcement>")]
pub async fn create_announcement_api(
	state: &State,
	session_id: SessionID<'_>,
	announcement: Form<AnnouncementForm>,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Creating announcement");
	let _enter = span.enter();

	session_id.verify_elevated(state).await?;

	let id = generate_id();
	let body = if announcement.body.is_empty() {
		None
	} else {
		Some(announcement.body.clone())
	};
	let date = Utc::now().to_rfc2822();

	let mentioned = announcement
		.mentioned
		.iter()
		.map(|x| MemberMention::from_str(x).unwrap())
		.collect();

	let new_announcement = Announcement {
		id,
		title: announcement.title.clone(),
		date,
		body,
		event: None,
		mentioned,
		read: HashSet::new(),
	};

	{
		let mut lock = state.db.lock().await;
		lock.create_announcement(new_announcement)
			.await
			.map_err(|e| {
				error!("Failed to create announcement: {}", e);
				Status::InternalServerError
			})?;
	}

	Ok(())
}

#[derive(FromForm)]
pub struct AnnouncementForm {
	title: String,
	body: String,
	mentioned: Vec<String>,
}

#[rocket::get("/create_announcement")]
pub async fn create_announcement_page(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Create announcement page");
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

	if !member.is_elevated() {
		error!("Invalid permissions");
		return Ok(redirect);
	}

	let lock = state.db.lock().await;

	let page = include_str!("pages/announcements/create_announcement.min.html");

	// Generate mention checkboxes
	let mut mentions_string = String::new();
	let mut available_mentions = Vec::new();
	for group in MemberGroup::iter() {
		available_mentions.push((
			format!("@{}", group.to_string()),
			format!(
				"<div class=\"group-mention-label\">{}</div>",
				group.to_plural_string().to_string()
			),
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
		available_mentions.push((id, member.name));
	}

	for (i, (mention, mention_pretty)) in available_mentions.into_iter().enumerate() {
		let label = format!("<label for=\"{mention}\">{mention_pretty}</label>");
		let checkbox =
			format!("<input type=checkbox name=\"{mention}\" id=mention-checkbox-{i} />");

		let mention = format!("<div class=\"cont mention-checkbox\">{label}{checkbox}</div>");

		mentions_string.push_str(&mention);
	}
	let page = page.replace("{{mentions}}", &mentions_string);

	let page = create_page("Create Announcement", &page);

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

#[rocket::get("/announcement/<id>")]
pub async fn announcement_details(
	session_id: OptionalSessionID<'_>,
	state: &State,
	id: &str,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Announcement details");
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

	let announcement = state
		.db
		.lock()
		.await
		.get_announcement(id)
		.await
		.map_err(|e| {
			error!("Failed to get announcement {id}: {e}");
			Status::InternalServerError
		})?;

	let Some(announcement) = announcement else {
		error!("Announcement {id} does not exist");
		return Err(Status::NotFound);
	};

	if !announcement.can_member_see(&member) {
		error!("Member cannot see announcement");
		return Err(Status::NotFound);
	}

	let page = include_str!("pages/announcements/details.min.html");
	let page = page.replace("{{title}}", &announcement.title);
	let date = DateTime::parse_from_rfc2822(&announcement.date)
		.map(|x| render_date(x))
		.unwrap_or("Invalid Date".into());
	let page = page.replace("{{date}}", &date);

	let body = comrak::markdown_to_html(
		&announcement.body.unwrap_or_default(),
		&comrak::Options::default(),
	);
	let page = page.replace("{{body}}", &body);

	let page = create_page("Announcement", &page);

	Ok(PageOrRedirect::Page(RawHtml(page)))
}
