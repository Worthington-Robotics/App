use chrono::DateTime;
use rocket::http::Status;
use rocket::response::{content::RawHtml, Redirect};
use tracing::{error, span, Level};

use crate::announcements::Announcement;
use crate::db::Database;
use crate::render_date;
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
		lock.get_member(&requesting_member_id)
	}) else {
		error!("Unknown requesting member ID {}", requesting_member_id);
		return Ok(redirect);
	};

	let page = include_str!("pages/inbox.min.html");

	let lock = state.db.lock().await;
	let mut announcements_string = String::with_capacity(
		include_str!("components/announcement.min.html").len() * lock.get_announcements().count(),
	);

	for announcement in lock.get_announcements() {
		announcements_string.push_str(&render_announcement(announcement));
	}
	let page = page.replace("{{announcements}}", &announcements_string);

	let page = create_page("Inbox", &page);

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

fn render_announcement(announcement: &Announcement) -> String {
	let component = include_str!("components/announcement.min.html");
	let date = DateTime::parse_from_rfc2822(&announcement.date)
		.map(render_date)
		.unwrap_or("Invalid date".into());
	let out = component.replace("{{date}}", &date);
	let out = out.replace("{{title}}", &announcement.title);
	let out = out.replace("{{body}}", &announcement.body.clone().unwrap_or_default());

	out
}
