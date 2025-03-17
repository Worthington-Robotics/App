pub mod review;
pub mod schedule;

use chrono::Utc;
use rocket::{
	form::{Form, FromForm},
	http::Status,
	response::{content::RawHtml, Redirect},
};
use tracing::{error, span, Level};

use crate::{
	db::Database,
	events::get_season,
	routes::{create_page, OptionalSessionID, PageOrRedirect, Scope, SessionID},
	scouting::{
		matches::{MatchStats, MatchStatsID},
		status::{RobotStatus, StatusUpdate},
		Competition, TeamNumber,
	},
	util::ToDropdown,
	State,
};

use super::download::Downloadable;

#[rocket::post("/api/post_match_stats", data = "<stats>")]
pub async fn create_match_stats(
	state: &State,
	session_id: SessionID<'_>,
	stats: Form<StatsForm>,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Creating match stats");
	let _enter = span.enter();

	let requesting_member = session_id.get_requesting_member(state).await?;

	let stats_id = stats.stats_id.clone().filter(|x| !x.is_empty());

	let mut stats: MatchStats = serde_json::from_str(&stats.data).map_err(|e| {
		error!("Invalid match stats data: {e}");
		error!("{}", &stats.data);
		Status::BadRequest
	})?;

	let now = Utc::now().to_rfc2822();

	let mut lock = state.db.write().await;

	// Fill out record info if it's empty
	if stats.recorder.is_none() {
		stats.recorder = Some(requesting_member.id.clone());
	}
	if stats.record_time.is_none() {
		stats.record_time = Some(now.clone());
	}
	if stats.competition.is_none() {
		let global_data = lock.get_global_data().await.map_err(|e| {
			error!("Failed to get global data from database: {e}");
			Status::InternalServerError
		})?;
		stats.competition = global_data.current_competition;
	}

	// If the report was posted live, then update robot status. We only add a good status update if the robot wasn't good before
	if stats.recorded_live {
		let status_updates = lock.get_team_status(stats.team_number).await;
		match status_updates {
			Ok(status_updates) => {
				let current_status = RobotStatus::get_from_updates(status_updates.iter());
				if stats.status != RobotStatus::Good || current_status != RobotStatus::Good {
					let update = StatusUpdate {
						team: stats.team_number,
						date: now,
						status: stats.status,
						details: stats.notes.clone(),
						member: requesting_member.id.clone(),
						competition: stats.competition.clone(),
					};

					// Not a super bad error, it's more important that the stats get posted
					if let Err(e) = lock.update_team_status(update).await {
						error!("Failed to create status update in database: {e}");
					}
				}
			}
			Err(e) => {
				error!("Failed to get status updates from database: {e}");
			}
		}
	}

	// If there is a stats ID, we are replacing an existing stats report and need to remove it
	if let Some(stats_id) = stats_id {
		let id = MatchStatsID::from_str(stats_id);
		if let Err(e) = lock.delete_match_stats(&id).await {
			error!("Failed to delete existing match stats with id {id} in database: {e}");
			return Err(Status::InternalServerError);
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
	stats_id: Option<String>,
}

#[rocket::delete("/api/delete_match_stats/<id>")]
pub async fn delete_match_stats(
	state: &State,
	session_id: SessionID<'_>,
	id: &str,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Deleting match stats");
	let _enter = span.enter();

	session_id.get_requesting_member(state).await?;

	let mut lock = state.db.write().await;

	if let Err(e) = lock
		.delete_match_stats(&MatchStatsID::from_str(id.to_string()))
		.await
	{
		error!("Failed to delete match stats {id} in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

/// Form for match reporting will all the bells and whistles
#[rocket::get("/scouting/report?<team_number>&<match_number>&<competition>&<stats_id>")]
pub async fn match_report_main(
	session_id: OptionalSessionID<'_>,
	state: &State,
	team_number: Option<TeamNumber>,
	match_number: Option<&str>,
	competition: Option<&str>,
	stats_id: Option<&str>,
) -> Result<PageOrRedirect, Status> {
	match_report_main_impl(
		session_id,
		state,
		team_number,
		match_number,
		competition,
		stats_id,
	)
	.await
}

/// Downloadable scouting report
#[rocket::get("/scouting/report.html")]
pub async fn match_report_download(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<Downloadable, Status> {
	let out = match_report_main_impl(session_id, state, None, None, None, None).await?;
	let PageOrRedirect::Page(mut data) = out else {
		return Err(Status::Unauthorized);
	};

	// Inline stylesheets and scripts
	let css = include_str!("../../../assets/static.min.css");
	let css = format!("<style>{css}</style>");
	data.0.push_str(&css);
	let js = include_str!("../../../assets/error.js");
	let js = format!("<script>{js}</script>");
	data.0.push_str(&js);
	let js = include_str!("../../../assets/scripts/prompt.js");
	let js = format!("<script>{js};loadPrompt();</script>");
	data.0.push_str(&js);

	let out = data.0.as_bytes().to_vec();

	Ok(Downloadable(out))
}

pub async fn match_report_main_impl(
	session_id: OptionalSessionID<'_>,
	state: &State,
	team_number: Option<TeamNumber>,
	match_number: Option<&str>,
	competition: Option<&str>,
	stats_id: Option<&str>,
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

	let page = include_str!("../../pages/scouting/report/main.min.html");
	let page = page.replace(
		"{team-number}",
		&if let Some(team_number) = team_number {
			team_number.to_string()
		} else {
			String::new()
		},
	);
	let page = page.replace("{match-number}", match_number.unwrap_or_default());

	let lock = state.db.read().await;

	let mut competition = competition.unwrap_or_default().to_string();
	// If the competition is "current", replace it with whatever the current competition is
	if competition == "current" {
		let global_data = lock.get_global_data().await.map_err(|e| {
			error!("Failed to get global data from database: {e}");
			Status::InternalServerError
		})?;
		competition = global_data
			.current_competition
			.unwrap_or(Competition::Pittsburgh)
			.to_string();
	}

	let parsed_competition = Competition::from_db(&competition);

	let options = Competition::create_options(parsed_competition.as_ref());
	let options = format!("<option value=none>None</option>{options}");
	let page = page.replace("{{competition-options}}", &options);

	let stats_id = stats_id.filter(|x| !x.is_empty());
	let page = page.replace("{{stats-id}}", stats_id.unwrap_or_default());

	let page = page.replace("{{frc-season}}", &get_season(&Utc::now()).to_string());

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

	let page = include_str!("../../pages/scouting/report/raw.min.html");

	let page = create_page("Raw Match Report", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

/// Workaround for CORS
#[rocket::delete("/api/check_tba_match/<match>")]
pub async fn check_tba_match(state: &State, r#match: &str) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Checking TBA match");
	let _enter = span.enter();

	let url = format!("https://www.thebluealliance.com/match/{match}");

	let result = state.req_client.get(url).send().await;
	let Ok(result) = result else {
		return Err(Status::InternalServerError);
	};
	if result.error_for_status().is_err() {
		return Err(Status::NotFound);
	}

	Ok(())
}
