use chrono::{DateTime, Datelike, FixedOffset, TimeZone, Utc};
use chrono_tz::US::Eastern;
use itertools::Itertools;
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
		matches::{Match, MatchNumber, MatchStats, MatchType},
		status::{RobotStatus, StatusUpdate},
		Competition, TeamNumber,
	},
	util::{date_from_js, render_time},
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

	let mut lock = state.db.write().await;

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

#[rocket::get("/scouting/schedule")]
pub async fn match_schedule(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Match schedule");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	let Ok(requesting_member) = session_id.get_requesting_member(state).await else {
		return Ok(redirect);
	};

	let page = include_str!("../pages/scouting/schedule.min.html");

	let lock = state.db.read().await;
	let matches = lock
		.get_matches()
		.await
		.map_err(|e| {
			error!("Failed to get matches from database: {e}");
			Status::InternalServerError
		})?
		.sorted_by_key(|x| x.num.num);

	let now = Utc::now();

	let mut matches_string = String::new();
	let mut last_date: Option<DateTime<FixedOffset>> = None;
	let mut day_counter = 1;
	// Whether the upcoming match was already chosen
	let mut next_chosen = false;
	for m in matches {
		// Insert break elements between days
		if let Some(Ok(date)) = m.date.as_ref().map(|x| DateTime::parse_from_rfc2822(&x)) {
			if let Some(last_date) = &last_date {
				if date.day() != last_date.day() {
					day_counter += 1;
					matches_string.push_str(&format!(
						"<div class=\"cont col day-break\">Day {day_counter}</div>"
					));
				}
			}
			last_date = Some(date);
		}

		matches_string.push_str(&render_match(m, &now, &mut next_chosen).await);
	}
	let page = page.replace("{{matches}}", &matches_string);

	let admin_control_style = if requesting_member.is_elevated() {
		""
	} else {
		"display:none"
	};
	let page = page.replace("{{admin-control-style}}", admin_control_style);

	let page = create_page("Match Schedule", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

async fn render_match(m: Match, now: &DateTime<Utc>, next_chosen: &mut bool) -> String {
	let out = include_str!("../components/scouting/match.min.html");
	let out = out.replace("{{number}}", &m.num.num.to_string());

	let is_our_match = m.red_alliance.contains(&4145) || m.blue_alliance.contains(&4145);

	let (date, next_class) =
		if let Some(Ok(date)) = m.date.map(|x| DateTime::parse_from_rfc2822(&x)) {
			let next_class = if !*next_chosen && is_our_match && date > *now {
				*next_chosen = true;
				"next"
			} else {
				""
			};
			(render_time(date.with_timezone(&Eastern)), next_class)
		} else {
			(String::new(), "")
		};
	let out = out.replace("{{time}}", &date);
	let out = out.replace("{{red1}}", &m.red_alliance[0].to_string());
	let out = out.replace("{{red2}}", &m.red_alliance[1].to_string());
	let out = out.replace("{{red3}}", &m.red_alliance[2].to_string());
	let out = out.replace("{{blue1}}", &m.blue_alliance[0].to_string());
	let out = out.replace("{{blue2}}", &m.blue_alliance[1].to_string());
	let out = out.replace("{{blue3}}", &m.blue_alliance[2].to_string());

	// Add a class to us to make us stand out
	let out = out.replace("{{red1-class}}", is_us_class(m.red_alliance[0]));
	let out = out.replace("{{red2-class}}", is_us_class(m.red_alliance[1]));
	let out = out.replace("{{red3-class}}", is_us_class(m.red_alliance[2]));
	let out = out.replace("{{blue1-class}}", is_us_class(m.blue_alliance[0]));
	let out = out.replace("{{blue2-class}}", is_us_class(m.blue_alliance[1]));
	let out = out.replace("{{blue3-class}}", is_us_class(m.blue_alliance[2]));

	let ours_class = if is_our_match { "" } else { "not-ours" };
	let out = out.replace("{{ours-class}}", ours_class);
	let out = out.replace("{{next-class}}", next_class);

	out
}

fn is_us_class(team: TeamNumber) -> &'static str {
	if team == 4145 {
		"us"
	} else {
		""
	}
}

#[rocket::post("/api/import_match_schedule")]
pub async fn import_match_schedule(state: &State, session_id: SessionID<'_>) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Importing matches");
	let _enter = span.enter();

	session_id.verify_elevated(state).await?;

	let mut lock = state.db.write().await;

	let global_data = lock.get_global_data().await.map_err(|e| {
		error!("Failed to get global data from database: {e}");
		Status::InternalServerError
	})?;

	let Some(current_competition) = global_data.current_competition else {
		error!("No current competition");
		return Ok(());
	};

	let event_code = if current_competition == Competition::Champs {
		let Some(current_division) = global_data.current_division else {
			error!("No current division");
			return Ok(());
		};

		current_division.get_code()
	} else {
		let Some(event_code) = current_competition.get_code() else {
			error!("Event does not have a code");
			return Ok(());
		};

		event_code
	};

	let first_matches = state
		.first_client
		.get_match_schedule(get_season(&Utc::now()) as i32, event_code)
		.await
		.map_err(|e| {
			error!("Failed to get match schedule from FIRST API: {e:#}");
			Status::InternalServerError
		})?;

	// Sanity check
	if first_matches.len() < 30 {
		error!("Not enough matches");
		return Err(Status::InternalServerError);
	}

	let mut matches = Vec::new();
	for m in first_matches {
		let Ok(date) = date_from_js(m.start_time, true) else {
			error!("Failed to parse date for match");
			continue;
		};
		// Interpret the date as being from the competition timezone
		let Some(date) = current_competition
			.get_timezone()
			.from_local_datetime(&date.naive_utc())
			.earliest()
		else {
			error!("Failed to convert timezone for match date");
			continue;
		};

		matches.push(Match {
			num: MatchNumber {
				num: m.match_number,
				ty: MatchType::Qualification,
			},
			date: Some(date.to_rfc2822()),
			red_alliance: vec![
				m.teams[0].team_number,
				m.teams[1].team_number,
				m.teams[2].team_number,
			],
			blue_alliance: vec![
				m.teams[3].team_number,
				m.teams[4].team_number,
				m.teams[5].team_number,
			],
		});
	}

	if let Err(e) = lock.clear_matches().await {
		error!("Failed to clear match schedule in database: {e}");
		return Err(Status::InternalServerError);
	}

	for m in matches {
		if let Err(e) = lock.create_match(m).await {
			error!("Failed to create match in database: {e}");
			return Err(Status::InternalServerError);
		}
	}

	// Remove all match claims as they won't be valid anymore
	if let Err(e) = lock.clear_match_claims().await {
		error!("Failed to clear all match claims: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

#[rocket::post("/api/clear_match_schedule")]
pub async fn clear_match_schedule(state: &State, session_id: SessionID<'_>) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Clearing matches");
	let _enter = span.enter();

	session_id.verify_elevated(state).await?;

	let mut lock = state.db.write().await;

	if let Err(e) = lock.clear_matches().await {
		error!("Failed to clear match schedule in database: {e}");
		return Err(Status::InternalServerError);
	}

	// Remove all match claims as they won't be valid anymore
	if let Err(e) = lock.clear_match_claims().await {
		error!("Failed to clear all match claims: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}
