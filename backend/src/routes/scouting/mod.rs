pub mod assignments;
pub mod autos;
pub mod matches;
pub mod matchup;
pub mod my_scouting;
pub mod status;
pub mod teams;

use std::{collections::HashSet, fmt::Display};

use anyhow::Context;
use chrono::Utc;
use rocket::{
	http::Status,
	response::{content::RawHtml, Redirect},
};
use tracing::{span, Level};

use crate::{
	api::first::FirstClient,
	db::{Database, DatabaseImpl},
	events::get_season,
	routes::OptionalSessionID,
	scouting::Team,
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
		};

		db.create_team(team)
			.await
			.context("Failed to create team")?;
	}

	Ok(())
}

// Functions for rendering stat cards

pub fn render_stat_card(title: &str, stat: impl Display, strong: bool) -> String {
	let out = include_str!("../components/scouting/stat_card.min.html");
	let out = out.replace("{{stat}}", &stat.to_string());
	let out = out.replace("{{title}}", title);
	let class = if strong { "strong" } else { "" };
	let out = out.replace("{{stat-class}}", class);

	out
}

pub fn render_stat_card_float(title: &str, stat: f32, strong: bool) -> String {
	render_stat_card(title, format!("{stat:.2}"), strong)
}

pub fn render_stat_card_pct(title: &str, stat: f32, strong: bool) -> String {
	render_stat_card(title, format!("{:.1}%", stat * 100.0), strong)
}

pub fn render_stat_card_optional(title: &str, stat: Option<impl Display>, strong: bool) -> String {
	if let Some(stat) = stat {
		render_stat_card(title, stat, strong)
	} else {
		render_stat_card(title, "?", strong)
	}
}

pub fn render_stat_card_optional_bool(title: &str, stat: Option<bool>, strong: bool) -> String {
	if let Some(stat) = stat {
		render_stat_card(title, if stat { "Yes" } else { "No" }, strong)
	} else {
		render_stat_card(title, "?", strong)
	}
}

pub fn render_stat_card_optional_float(title: &str, stat: Option<f32>, strong: bool) -> String {
	if let Some(stat) = stat {
		render_stat_card_float(title, stat, strong)
	} else {
		render_stat_card(title, "?", strong)
	}
}
