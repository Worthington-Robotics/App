use rocket::response::content::RawHtml;
use rocket::{http::Status, response::Redirect};
use tracing::{error, span, Level};

use crate::db::Database;
use crate::{routes::OptionalSessionID, State};

use super::{create_page, PageOrRedirect};

#[rocket::get("/settings")]
pub async fn settings(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Settings page");
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

	let page = include_str!("pages/settings.min.html");
	let page = page.replace("{{name}}", &member.name);

	// Replace the calendar ID for the copy button
	let page = page.replace("{{cal-id}}", &member.calendar_id);
	let page = create_page("Settings", &page, Some(super::Scope::Home));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}
