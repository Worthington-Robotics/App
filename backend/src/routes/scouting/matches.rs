use rocket::{
	form::{Form, FromForm},
	http::Status,
	response::{content::RawHtml, Redirect},
};
use tracing::{error, span, Level};

use crate::{
	db::Database,
	routes::{create_page, OptionalSessionID, PageOrRedirect, Scope, SessionID},
	State,
};

#[rocket::post("/api/post_match_stats", data = "<stats>")]
pub async fn create_match_stats(
	state: &State,
	session_id: SessionID<'_>,
	stats: Form<StatsForm>,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Creating match stats");
	let _enter = span.enter();

	session_id.get_requesting_member(state).await?;

	let stats = serde_json::from_str(&stats.data).map_err(|e| {
		error!("Invalid match stats data: {e}");
		Status::BadRequest
	})?;

	let mut lock = state.db.lock().await;

	if let Err(e) = lock.create_match_stats(stats).await {
		error!("Failed to create match stats in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

#[derive(FromForm)]
pub struct StatsForm {
	data: String,
}

/// Form for match reporting will all the bells and whistles
#[rocket::get("/scouting/report")]
pub async fn match_report_main(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Match report");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.get_requesting_member(state).await.is_err() {
		return Ok(redirect);
	};

	let page = include_str!("../pages/scouting/report/main.min.html");

	let page = create_page("Match Report", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

/// Raw form for match reporting without any fancy features like timing, video, or auto drawing
#[rocket::get("/scouting/report/raw")]
pub async fn match_report_raw(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Raw match report");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.get_requesting_member(state).await.is_err() {
		return Ok(redirect);
	};

	let page = include_str!("../pages/scouting/report/raw.min.html");

	let page = create_page("Raw Match Report", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}
