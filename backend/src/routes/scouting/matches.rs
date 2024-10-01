use chrono::Utc;
use rocket::{
	form::{Form, FromForm},
	http::Status,
	response::{content::RawHtml, Redirect},
};
use tracing::{error, span, Level};

use crate::{
	db::Database,
	routes::{create_page, OptionalSessionID, PageOrRedirect, Scope, SessionID},
	scouting::{
		matches::MatchStats,
		status::{RobotStatus, StatusUpdate},
	},
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

	let requesting_member = session_id.get_requesting_member(state).await?;

	let mut stats: MatchStats = serde_json::from_str(&stats.data).map_err(|e| {
		error!("Invalid match stats data: {e}");
		Status::BadRequest
	})?;

	let now = Utc::now().to_rfc2822();

	// Fill out record info
	stats.recorder = Some(requesting_member.id.clone());
	stats.record_time = Some(now.clone());

	let mut lock = state.db.lock().await;

	// If the report was posted live, then update robot status. We only add a good status update if the robot wasn't good before
	if stats.recorded_live {
		let status_updates = lock.get_team_status(stats.team_number).await.map_err(|e| {
			error!("Failed to get status updates from database: {e}");
			Status::InternalServerError
		})?;
		let current_status = RobotStatus::get_from_updates(&status_updates);
		if stats.status != RobotStatus::Good || current_status != RobotStatus::Good {
			let update = StatusUpdate {
				team: stats.team_number,
				date: now,
				status: stats.status,
				details: stats.notes.clone(),
				member: requesting_member.id.clone(),
			};

			// Not a super bad error, it's more important that the stats get posted
			if let Err(e) = lock.update_team_status(update).await {
				error!("Failed to create status update in database: {e}");
			}
		}
	}

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
