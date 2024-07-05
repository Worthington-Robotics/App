use rocket::http::Status;
use rocket::response::{content::RawHtml, Redirect};
use tracing::{error, span, Level};

use crate::db::Database;
use crate::{routes::OptionalSessionID, State};

use super::{create_page, PageOrRedirect};

#[rocket::get("/inbox")]
pub async fn inbox(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Index");
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

	let page = create_page("Inbox", include_str!("pages/inbox.min.html"));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}
