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
	Responder,
};
use rocket_async_compression::{Compress, Level as CompressionLevel};
use serde::Serialize;
use strum::IntoEnumIterator;
use tracing::{error, span, Level};

use crate::{
	api::statbotics::StatboticsClient,
	db::{Database, DatabaseImpl},
	routes::{assets::CacheFor, OptionalSessionID, SessionID},
	scouting::{stats::CombinedTeamStats, status::RobotStatus, Competition, Team, TeamNumber},
	State,
};

use super::{
	create_page,
	stats::{create_stat_dropdown_options, StatInfo},
	PageOrRedirect, Scope,
};

#[rocket::get("/scouting/teams?<competition>")]
pub async fn teams(
	session_id: OptionalSessionID<'_>,
	state: &State,
	competition: Option<&str>,
) -> Result<OptionalCacheFor<Compress<PageOrRedirect>>, Status> {
	let span = span!(Level::DEBUG, "Teams");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(OptionalCacheFor::NoCache(Compress(
			redirect,
			CompressionLevel::Fastest,
		)));
	};

	let Ok(requesting_member) = session_id.get_requesting_member(state).await else {
		return Ok(OptionalCacheFor::NoCache(Compress(
			redirect,
			CompressionLevel::Fastest,
		)));
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
		teams_string.push_str(
			&render_team(
				team,
				&requesting_member.id,
				&state.statbotics_client,
				lock.deref(),
			)
			.await,
		);
	}
	let page = page.replace("{{teams}}", &teams_string);

	let mut comps_string = String::new();
	// Loop over all competitions along with the option for all teams and the current competition
	for (data, disp) in
		std::iter::once(("", "All")).chain(Competition::iter().map(|x| (x.into(), x.get_abbr())))
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

	// Stat dropdown
	let dropdown_options = create_stat_dropdown_options();
	let page = page.replace("{{stat-options}}", &dropdown_options);

	let page = create_page("Teams", &page, Some(Scope::Scouting));

	let should_cache = parsed_competition.is_none();

	let out = Compress(
		PageOrRedirect::Page(RawHtml(page)),
		CompressionLevel::Fastest,
	);

	if should_cache {
		Ok(OptionalCacheFor::Cache(CacheFor(out, 100)))
	} else {
		Ok(OptionalCacheFor::NoCache(out))
	}
}

#[derive(Responder)]
pub enum OptionalCacheFor<R> {
	Cache(CacheFor<R>),
	NoCache(R),
}

async fn render_team(
	team: Team,
	requesting_member: &str,
	stat_client: &StatboticsClient,
	db: &DatabaseImpl,
) -> String {
	let out = include_str!("../../components/scouting/team_row.min.html");
	let out = out.replace("{{number}}", &team.number.to_string());

	let number_class = if team.followers.contains(requesting_member) {
		"fave"
	} else {
		""
	};
	let out = out.replace("{{number-class}}", number_class);

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
	let Some(stat_info) = StatInfo::get(stat) else {
		error!("Stat not found: {stat}");
		return Err(Status::NotFound);
	};

	let mut data = Vec::new();
	for (i, stats) in team_stats.historical.iter().enumerate() {
		let value = StatInfo::get_stat_value(stats, stat)
			.expect("Should have already errored out if the field didn't exist");

		data.push(HistoricalPoint {
			r#match: i as u16,
			value,
		});
	}

	let out = HistoricalStatResult {
		stat_description: stat_info.description,
		data,
	};

	let out = serde_json::to_string(&out).map_err(|e| {
		error!("Failed to serialize output: {e}");
		Status::InternalServerError
	})?;

	Ok(RawJson(out))
}

#[derive(Serialize)]
pub struct HistoricalStatResult {
	pub stat_description: &'static str,
	pub data: Vec<HistoricalPoint>,
}

#[derive(Serialize)]
pub struct HistoricalPoint {
	r#match: u16,
	value: f32,
}

/// Get the chartsjs consumable list of historical data points for a stat
#[rocket::get("/api/get_teams_stat/<stat>?<competition>")]
pub async fn get_teams_stat(
	state: &State,
	session_id: SessionID<'_>,
	competition: &str,
	stat: &str,
) -> Result<Compress<RawJson<String>>, Status> {
	let span = span!(Level::DEBUG, "Getting stats for all teams");
	let _enter = span.enter();

	session_id.get_requesting_member(state).await?;

	let lock = state.db.read().await;

	let mut competition = competition.to_string();
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

	let teams = lock.get_teams().await.map_err(|e| {
		error!("Failed to get all teams from database: {e}");
		Status::InternalServerError
	})?;

	let teams = teams.filter(|x| {
		if let Some(competition) = parsed_competition {
			x.competitions.contains(&competition)
		} else {
			true
		}
	});

	let stats_lock = state.team_stats.read().await;

	let mut data = HashMap::with_capacity(2000);
	let default_stats = CombinedTeamStats::default();
	for team in teams {
		let team_stats = stats_lock.get(&team.number).unwrap_or(&default_stats);
		let team_stats = if competition == "Current" {
			&team_stats.current_competition
		} else {
			&team_stats.all_time
		};
		let stat = StatInfo::get_stat_value(team_stats, stat).unwrap_or_default();
		let stat = format!("{stat:.2}");
		data.insert(team.number, stat);
	}

	#[derive(Serialize)]
	struct Out {
		abbreviation: &'static str,
		data: HashMap<TeamNumber, String>,
	}

	let out = Out {
		abbreviation: StatInfo::get(stat)
			.map(|x| x.abbreviation)
			.unwrap_or("Stat"),
		data,
	};

	let out = serde_json::to_string(&out).map_err(|e| {
		error!("Failed to serialize output: {e}");
		Status::InternalServerError
	})?;

	Ok(Compress(RawJson(out), CompressionLevel::Fastest))
}
