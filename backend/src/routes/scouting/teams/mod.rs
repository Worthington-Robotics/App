pub mod details;
pub mod info;

use std::{collections::HashMap, ops::Deref};

use itertools::Itertools;
use rocket::{
	http::Status,
	response::{
		content::{RawHtml, RawJson},
		Redirect,
	},
};
use rocket_async_compression::{Compress, Level as CompressionLevel};
use serde::Serialize;
use strum::IntoEnumIterator;
use tracing::{error, span, Level};

use crate::{
	api::statbotics::StatboticsClient,
	db::{Database, DatabaseImpl},
	routes::{OptionalSessionID, SessionID},
	scouting::{stats::CombinedTeamStats, status::RobotStatus, Competition, Team, TeamNumber},
	State,
};

use super::{create_page, PageOrRedirect, Scope};

#[rocket::get("/scouting/teams?<competition>")]
pub async fn teams(
	session_id: OptionalSessionID<'_>,
	state: &State,
	competition: Option<&str>,
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

	let lock = state.db.read().await;

	let mut competition = competition.unwrap_or_default().to_string();
	// If the competition is "current", replace it with whatever the current competition is
	if competition == "Current" {
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

	let page = include_str!("../../pages/scouting/teams.min.html");

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
		teams_string.push_str(&render_team(team, &state.statbotics_client, lock.deref()).await);
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

	let page = page.replace("{{competition}}", &competition);

	let page = create_page("Teams", &page, Some(Scope::Scouting));

	Ok(Compress(
		PageOrRedirect::Page(RawHtml(page)),
		CompressionLevel::Fastest,
	))
}

async fn render_team(team: Team, stat_client: &StatboticsClient, db: &DatabaseImpl) -> String {
	let out = include_str!("../../components/scouting/team_row.min.html");
	let out = out.replace("{{number}}", &team.number.to_string());
	let out = out.replace("{{name}}", &team.sanitized_name());
	let out = out.replace("{{data-name}}", &format!("\"{}\"", team.sanitized_name()));
	let epa = stat_client.get_epa(team.number).await.unwrap_or(0.0);
	let out = out.replace("{{epa}}", &format!("{epa:.2}"));

	let status = if let Ok(status_updates) = db.get_team_status(team.number).await {
		let status = RobotStatus::get_from_updates(status_updates.iter());
		if status == RobotStatus::Good {
			String::new()
		} else {
			format!(
				"<div class=\"cont round status\" style=\"background-color: {}\">{}</div>",
				status.get_color(),
				status.get_abbr()
			)
		}
	} else {
		error!("Failed to get team status from database");
		String::new()
	};
	let out = out.replace("{{status}}", &status);

	out
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

	let mut lock = state.db.write().await;
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

#[rocket::post("/api/update_team_following/<id>")]
pub async fn update_team_following(
	state: &State,
	session_id: SessionID<'_>,
	id: TeamNumber,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Updating team following");
	let _enter = span.enter();

	let requesting_member = session_id.get_requesting_member(state).await?;

	let mut lock = state.db.write().await;
	let Some(mut team) = lock.get_team(id).await.map_err(|e| {
		error!("Failed to get team from database: {e}");
		Status::InternalServerError
	})?
	else {
		error!("Team {id} does not exist");
		return Err(Status::NotFound);
	};

	if team.followers.contains(&requesting_member.id) {
		team.followers.remove(&requesting_member.id);
	} else {
		team.followers.insert(requesting_member.id);
	}

	if let Err(e) = lock.create_team(team).await {
		error!("Failed to update team {id} in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

/// Get the chartsjs consumable list of historical data points for a stat
#[rocket::get("/api/get_historical_stat/<team>/<stat>")]
pub async fn get_historical_stat(
	state: &State,
	session_id: SessionID<'_>,
	team: TeamNumber,
	stat: &str,
) -> Result<RawJson<String>, Status> {
	let span = span!(Level::DEBUG, "Getting historical stat");
	let _enter = span.enter();

	session_id.get_requesting_member(state).await?;

	let default_stats = CombinedTeamStats::default();
	let lock = state.team_stats.read().await;
	let team_stats = lock.get(&team).unwrap_or(&default_stats);

	/* We do this by converting the stats to a HashMap using serde, then looking for the field we want */

	// Ensure that this is a field in the stats
	let serialized_default =
		serde_json::to_string(&default_stats.all_time).expect("Failed to serialize default stats");
	let deserialized_default: HashMap<String, serde_json::Value> =
		serde_json::from_str(&serialized_default).expect("Failed to deserialize default stats");
	if !deserialized_default.contains_key(stat) {
		return Err(Status::NotFound);
	}

	#[derive(Serialize)]
	struct Point {
		r#match: u16,
		value: f64,
	}

	let mut out = Vec::new();
	for (i, m) in team_stats.historical.iter().enumerate() {
		let serialized = serde_json::to_string(m).expect("Failed to serialize match stats");
		let deserialized: HashMap<String, serde_json::Value> =
			serde_json::from_str(&serialized).expect("Failed to deserialize match stats");
		let value = deserialized
			.get(stat)
			.expect("Should have already errored out if the field didn't exist");
		let Some(value) = value.as_f64() else {
			continue;
		};

		out.push(Point {
			r#match: i as u16,
			value,
		});
	}

	let out = serde_json::to_string(&out).map_err(|e| {
		error!("Failed to serialize output: {e}");
		Status::InternalServerError
	})?;

	Ok(RawJson(out))
}
