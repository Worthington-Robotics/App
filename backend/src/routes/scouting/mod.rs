use std::collections::HashSet;

use anyhow::Context;
use chrono::Utc;
use itertools::Itertools;
use rocket::{
	http::Status,
	response::{content::RawHtml, Redirect},
};
use rocket_async_compression::{Compress, Level as CompressionLevel};
use tracing::{error, span, Level};

use crate::{
	api::first::FirstClient,
	db::{Database, DatabaseImpl},
	events::get_season,
	routes::OptionalSessionID,
	scouting::{Team, TeamNumber},
	State,
};

use super::{create_page, PageOrRedirect, Scope};

#[rocket::get("/scouting/teams?<all>")]
pub async fn teams(
	session_id: OptionalSessionID<'_>,
	state: &State,
	all: bool,
) -> Result<Compress<PageOrRedirect>, Status> {
	let span = span!(Level::DEBUG, "Teams");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(Compress(redirect, CompressionLevel::Fastest));
	};

	if session_id.get_requesting_member(state).await.is_err() {
		return Ok(Compress(redirect, CompressionLevel::Fastest));
	};

	let page = include_str!("../pages/scouting/teams.min.html");

	let lock = state.db.lock().await;
	let teams = lock
		.get_teams()
		.await
		.map_err(|e| {
			error!("Failed to get teams from database: {e}");
			Status::InternalServerError
		})?
		.sorted_by_key(|x| x.number);

	let mut teams_string = String::new();
	for team in teams {
		teams_string.push_str(&render_team(team));
	}
	let page = page.replace("{{teams}}", &teams_string);

	let page = create_page("Teams", &page, Some(Scope::Scouting));

	Ok(Compress(
		PageOrRedirect::Page(RawHtml(page)),
		CompressionLevel::Fastest,
	))
}

fn render_team(team: Team) -> String {
	let out = include_str!("../components/scouting/team_row.min.html");
	let out = out.replace("{{number}}", &team.number.to_string());
	let out = out.replace("{{name}}", &team.sanitized_name());

	out
}

#[rocket::get("/scouting/team/<id>")]
pub async fn team_details(
	id: TeamNumber,
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Team details page");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.get_requesting_member(state).await.is_err() {
		return Ok(redirect);
	};

	let lock = state.db.lock().await;
	let team = lock
		.get_team(id)
		.await
		.map_err(|e| {
			error!("Failed to get team from database: {e}");
			Status::InternalServerError
		})?
		.ok_or_else(|| {
			error!("Team does not exist: {}", id);
			Status::NotFound
		})?;

	let page = include_str!("../pages/scouting/team_details.min.html");
	let page = page.replace("{{name}}", &team.name);
	let page = page.replace("{{number}}", &team.number.to_string());
	let page = page.replace("{{rookie-year}}", &team.rookie_year.to_string());

	let page = create_page("Team Details", &page, Some(Scope::Scouting));

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
		};

		db.create_team(team)
			.await
			.context("Failed to create team")?;
	}

	Ok(())
}
