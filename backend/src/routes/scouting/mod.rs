/// Prescouting assignments and match claims
pub mod assignments;
pub mod autos;
pub mod matches;
pub mod matchup;
pub mod my_scouting;
/// Stat card rendering
mod stats;
pub mod status;
pub mod teams;

use std::{collections::HashSet, io::Cursor};

use anyhow::Context;
use chrono::Utc;
use rocket::{
	form::Form,
	http::Status,
	response::{content::RawHtml, Redirect},
	FromForm, Responder,
};
use tracing::{error, span, Level};

use crate::{
	api::first::FirstClient,
	db::{Database, DatabaseImpl},
	events::get_season,
	routes::{OptionalSessionID, SessionID},
	scouting::{stats::CombinedTeamStats, Competition, Division, Team},
	util::ToDropdown,
	State,
};

use super::{create_page, PageOrRedirect, Scope};

#[rocket::get("/scouting")]
pub async fn index(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Scouting index");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	let Ok(requesting_member) = session_id.get_requesting_member(state).await else {
		return Ok(redirect);
	};

	let page = include_str!("../pages/scouting/index.min.html");

	let admin_display = if requesting_member.is_elevated() {
		""
	} else {
		"none"
	};
	let page = page.replace("{{admin-display}}", admin_display);

	let page = create_page("Scouting", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

#[rocket::get("/scouting/admin")]
pub async fn admin(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Scouting admin page");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.verify_elevated(state).await.is_err() {
		return Ok(redirect);
	};

	let page = include_str!("../pages/scouting/admin.min.html");

	let lock = state.db.read().await;
	let data = lock.get_global_data().await.map_err(|e| {
		error!("Failed to get global data from database: {e}");
		Status::InternalServerError
	})?;

	let options = Competition::create_options(data.current_competition.as_ref());
	let options = format!("<option value=none>None</option>{options}");
	let page = page.replace("{{competition-options}}", &options);

	let options = Division::create_options(data.current_division.as_ref());
	let options = format!("<option value=none>None</option>{options}");
	let page = page.replace("{{division-options}}", &options);

	let page = create_page("Scouting Administration", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

/// Populate the database with teams from the API
pub async fn populate_teams(
	db: &mut DatabaseImpl,
	first_client: &FirstClient,
) -> anyhow::Result<()> {
	println!("Getting teams from API...");
	let teams = first_client
		.get_teams(get_season(&Utc::now()) as i32)
		.await
		.context("Failed to get teams from FIRST API")?;

	// Get the teams already existing in the database so then we don't recreate existing ones
	println!("Getting existing teams from database...");
	let existing_teams: HashSet<_> = db
		.get_teams()
		.await
		.context("Failed to get existing teams from database")?
		.map(|x| x.number)
		.collect();

	println!("Adding teams to database...");
	for team in teams {
		if existing_teams.contains(&team.team_number) {
			continue;
		}

		let team = Team {
			name: team.name_short,
			number: team.team_number,
			rookie_year: team.rookie_year,
			competitions: HashSet::new(),
			followers: HashSet::new(),
		};

		db.create_team(team)
			.await
			.context("Failed to create team")?;
	}

	Ok(())
}

#[rocket::get("/api/scouting_data.csv")]
pub async fn download_data(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<Downloadable, Status> {
	let span = span!(Level::DEBUG, "Downloading scouting data");
	let _enter = span.enter();

	let Some(session_id) = session_id.to_session_id() else {
		return Err(Status::Unauthorized);
	};

	if session_id.verify_elevated(state).await.is_err() {
		return Err(Status::Unauthorized);
	};

	let lock = state.db.read().await;
	let match_stats = lock.get_all_match_stats().await.map_err(|e| {
		error!("Failed to get match stats from database: {e}");
		Status::InternalServerError
	})?;

	let match_stats: Vec<_> = match_stats.collect();

	let teams = lock.get_teams().await.map_err(|e| {
		error!("Failed to get teams from database: {e}");
		Status::InternalServerError
	})?;

	let stats_lock = state.team_stats.read().await;

	let mut buf = Vec::new();
	let mut csv_writer = csv::Writer::from_writer(Cursor::new(&mut buf));

	let default_stats = CombinedTeamStats::default();
	for team in teams {
		// Don't include teams with no matches
		if !match_stats.iter().any(|x| x.team_number == team.number) {
			continue;
		}

		let stats = stats_lock.get(&team.number).unwrap_or(&default_stats);

		if let Err(e) = csv_writer.serialize(&stats.all_time) {
			error!("Failed to serialize row: {e}");
			continue;
		};
	}
	if let Err(e) = csv_writer.flush() {
		error!("Failed to flush CSV buffer: {e}");
		return Err(Status::InternalServerError);
	}

	std::mem::drop(csv_writer);

	Ok(Downloadable(buf))
}

/// Responder with a content type set to something nonsense so that browsers
/// won't render it and will download it instead
#[derive(Responder)]
#[response(content_type = "application/download-me")]
pub struct Downloadable(Vec<u8>);

#[rocket::post("/api/scouting/update_settings", data = "<settings>")]
pub async fn update_settings(
	session_id: SessionID<'_>,
	state: &State,
	settings: Form<SettingsForm>,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Updating scouting settings");
	let _enter = span.enter();

	session_id.verify_elevated(state).await?;

	let mut lock = state.db.write().await;

	let mut current_data = lock.get_global_data().await.map_err(|e| {
		error!("Failed to get global data from database: {e}");
		Status::InternalServerError
	})?;

	let competition = if settings.competition == "none" {
		None
	} else {
		Some(Competition::from_db(&settings.competition).ok_or_else(|| {
			error!("Invalid competition");
			Status::BadRequest
		})?)
	};

	let division = if settings.division == "none" {
		None
	} else {
		Some(Division::from_db(&settings.division).ok_or_else(|| {
			error!("Invalid division");
			Status::BadRequest
		})?)
	};

	current_data.current_competition = competition;
	current_data.current_division = division;

	if let Err(e) = lock.set_global_data(current_data).await {
		error!("Failed to set global data in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

#[derive(FromForm)]
pub struct SettingsForm {
	pub competition: String,
	pub division: String,
}
