pub mod matches;

use std::{collections::HashSet, fmt::Display};

use anyhow::Context;
use chrono::Utc;
use itertools::Itertools;
use rocket::{
	http::Status,
	response::{content::RawHtml, Redirect},
};
use rocket_async_compression::{Compress, Level as CompressionLevel};
use strum::IntoEnumIterator;
use tracing::{error, span, Level};

use crate::{
	api::{first::FirstClient, statbotics::StatboticsClient},
	db::{Database, DatabaseImpl},
	events::get_season,
	routes::{OptionalSessionID, SessionID},
	scouting::{Competition, Team, TeamNumber, TeamStats},
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

	if session_id.get_requesting_member(state).await.is_err() {
		return Ok(redirect);
	};

	let page = include_str!("../pages/scouting/index.min.html");

	let page = create_page("Scouting", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

#[rocket::get("/scouting/teams?<competition>")]
pub async fn teams(
	session_id: OptionalSessionID<'_>,
	state: &State,
	competition: Option<&str>,
) -> Result<Compress<PageOrRedirect>, Status> {
	let span = span!(Level::DEBUG, "Teams");
	let _enter = span.enter();

	let mut competition = competition.unwrap_or_default();
	// If the competition is "current", replace it with whatever the current competition is
	if competition == "Current" {
		// TODO: Use the actual current competition
		competition = "Pittsburgh";
	}

	let parsed_competition = Competition::from_db(competition);

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
		// Skip teams that aren't at the given competition
		if let Some(competition) = &parsed_competition {
			if !team.competitions.contains(competition) {
				continue;
			}
		}
		teams_string.push_str(&render_team(team, &state.statbotics_client).await);
	}
	let page = page.replace("{{teams}}", &teams_string);

	let mut comps_string = String::new();
	// Loop over all competitions along with the option for all teams and the current competition
	for (data, disp) in Competition::iter()
		.map(|x| (x.into(), x.get_abbr()))
		.chain(std::iter::once(("", "All")))
	{
		let is_selected = if data.is_empty() {
			competition.is_empty()
		} else {
			competition == data
		};
		let selected_class = if is_selected { " selected" } else { "" };
		let additional_class = if data.is_empty() { " all" } else { "" };

		let elem = format!(
			r#"<a href=/scouting/teams?competition={data} class="round cont nolink comp{selected_class}{additional_class}"><button>{disp}</button></a>"#
		);

		comps_string.push_str(&elem);
	}
	let page = page.replace("{{comp-options}}", &comps_string);

	let page = create_page("Teams", &page, Some(Scope::Scouting));

	Ok(Compress(
		PageOrRedirect::Page(RawHtml(page)),
		CompressionLevel::Fastest,
	))
}

async fn render_team(team: Team, stat_client: &StatboticsClient) -> String {
	let out = include_str!("../components/scouting/team_row.min.html");
	let out = out.replace("{{number}}", &team.number.to_string());
	let out = out.replace("{{name}}", &team.sanitized_name());
	let epa = stat_client.get_epa(team.number).await.unwrap_or(0.0);
	let out = out.replace("{{epa}}", &format!("{epa:.2}"));

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

	let Ok(requesting_member) = session_id.get_requesting_member(state).await else {
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

	// Create checkboxes for changing competition status
	let disabled_attr = if requesting_member.is_elevated() {
		""
	} else {
		" disabled"
	};
	let mut checkboxes_string = String::new();
	for comp in Competition::iter() {
		let checked_attr = if team.competitions.contains(&comp) {
			" checked"
		} else {
			""
		};

		let component = format!(
			r#"<div class="cont comp-cb"><input type=checkbox {disabled_attr} {checked_attr} data-val={comp} /> {}</div>"#,
			comp.get_abbr()
		);
		checkboxes_string.push_str(&component);
	}
	let page = page.replace("{{comp-checkboxes}}", &checkboxes_string);

	// Create stats
	let epa = state
		.statbotics_client
		.get_epa(id)
		.await
		.unwrap_or_default();
	let page = page.replace("{{epa}}", &render_stat_card_float("EPA", epa, true));
	let default_stats = TeamStats::default();
	let lock2 = state.team_stats.read().await;
	let team_stats = lock2.get(&id).unwrap_or(&default_stats);
	let page = page.replace(
		"{{apa}}",
		&render_stat_card_float("APA", team_stats.apa, true),
	);
	let page = page.replace(
		"{{win-rate}}",
		&render_stat_card_pct("Win Rate", team_stats.win_rate, true),
	);
	let page = page.replace(
		"{{matches}}",
		&render_stat_card("Matches", team_stats.matches, false),
	);
	let page = page.replace(
		"{{availability}}",
		&render_stat_card_pct("Availability", team_stats.availablity, false),
	);
	let page = page.replace(
		"{{penalties}}",
		&render_stat_card("Penalties", team_stats.penalties, false),
	);
	let page = page.replace(
		"{{auto-score}}",
		&render_stat_card_float("Score", team_stats.auto_score, true),
	);
	let page = page.replace(
		"{{auto-accuracy}}",
		&render_stat_card_float("Accuracy", team_stats.auto_accuracy, true),
	);
	let page = page.replace(
		"{{auto-collisions}}",
		&render_stat_card("Collisions", team_stats.auto_collisions, false),
	);
	let page = page.replace(
		"{{cycle-time}}",
		&render_stat_card_float("CT", team_stats.cycle_time, true),
	);
	let page = page.replace(
		"{{speaker-score}}",
		&render_stat_card_float("Spkr Sco", team_stats.speaker_score, false),
	);
	let page = page.replace(
		"{{amp-score}}",
		&render_stat_card_float("Amp Sco", team_stats.amp_score, false),
	);
	let page = page.replace(
		"{{pass-average}}",
		&render_stat_card_float("Pass Avg", team_stats.pass_average, false),
	);
	let page = page.replace(
		"{{speaker-accuracy}}",
		&render_stat_card_pct("Spkr Acc", team_stats.speaker_accuracy, false),
	);
	let page = page.replace(
		"{{amp-accuracy}}",
		&render_stat_card_pct("Amp Acc", team_stats.amp_accuracy, false),
	);
	let page = page.replace(
		"{{amp-rate}}",
		&render_stat_card_float("Amp Rate", team_stats.amplification_rate, true),
	);
	let page = page.replace(
		"{{amp-power}}",
		&render_stat_card_float("Amp Pwr", team_stats.amplification_power, true),
	);
	let page = page.replace(
		"{{defense-average}}",
		&render_stat_card_float("Def Avg", team_stats.defense_average, false),
	);
	let page = page.replace(
		"{{climb-score}}",
		&render_stat_card_float("Climb Sco", team_stats.climb_score, true),
	);
	let page = page.replace(
		"{{climb-accuracy}}",
		&render_stat_card_pct("Climb Acc", team_stats.climb_accuracy, false),
	);
	let page = page.replace(
		"{{trap-score}}",
		&render_stat_card_float("Trap Sco", team_stats.trap_score, true),
	);
	let page = page.replace(
		"{{trap-accuracy}}",
		&render_stat_card_pct("Trap Acc", team_stats.trap_accuracy, false),
	);

	let page = create_page("Team Details", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

fn render_stat_card(title: &str, stat: impl Display, strong: bool) -> String {
	let out = include_str!("../components/scouting/stat_card.min.html");
	let out = out.replace("{{stat}}", &stat.to_string());
	let out = out.replace("{{title}}", title);
	let class = if strong { "strong" } else { "" };
	let out = out.replace("{{stat-class}}", class);

	out
}

fn render_stat_card_float(title: &str, stat: f32, strong: bool) -> String {
	render_stat_card(title, format!("{stat:.2}"), strong)
}

fn render_stat_card_pct(title: &str, stat: f32, strong: bool) -> String {
	render_stat_card(title, format!("{:.1}%", stat * 100.0), strong)
}

#[rocket::post("/api/update_team_competition/<id>?<competition>")]
pub async fn update_team_competition(
	state: &State,
	session_id: SessionID<'_>,
	id: TeamNumber,
	competition: String,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Updating team competition");
	let _enter = span.enter();

	session_id.verify_elevated(state).await?;

	let mut lock = state.db.lock().await;
	let Some(mut team) = lock.get_team(id).await.map_err(|e| {
		error!("Failed to get team from database: {e}");
		Status::InternalServerError
	})?
	else {
		error!("Team {id} does not exist");
		return Err(Status::NotFound);
	};

	let Some(competition) = Competition::from_db(&competition) else {
		error!("Unknown competition {competition}");
		return Err(Status::BadRequest);
	};
	if team.competitions.contains(&competition) {
		team.competitions.remove(&competition);
	} else {
		team.competitions.insert(competition);
	}

	if let Err(e) = lock.create_team(team).await {
		error!("Failed to update team {id} in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
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
