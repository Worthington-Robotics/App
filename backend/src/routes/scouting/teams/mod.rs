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
	scouting::{
		stats::CombinedTeamStats, status::RobotStatus, ClimbAbility, Competition, DriveTrainType, GamePiece, ReefLevel, Team, TeamNumber
	},
	State,
};

use super::{
	create_page,
	stats::{
		render_stat_card_float, render_stat_card_optional, render_stat_card_optional_bool,
		render_stat_card_optional_float, stat_card_float, stat_card_other, stat_card_pct,
	},
	PageOrRedirect, Scope,
};

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
		let status = RobotStatus::get_from_updates(&status_updates);
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

#[rocket::get("/scouting/team/<id>?<competition>")]
pub async fn team_details(
	id: TeamNumber,
	session_id: OptionalSessionID<'_>,
	state: &State,
	competition: Option<&str>,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Team details page");
	let _enter = span.enter();

	let competition_str = competition.unwrap_or_default();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	let Ok(requesting_member) = session_id.get_requesting_member(state).await else {
		return Ok(redirect);
	};

	let lock = state.db.read().await;
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

	let page = include_str!("../../pages/scouting/team/details.min.html");
	let page = page.replace("{{name}}", &team.name);
	let page = page.replace("{{number}}", &team.number.to_string());
	let page = page.replace("__team_number__", &team.number.to_string());
	let page = page.replace("{{rookie-year}}", &team.rookie_year.to_string());
	let page = page.replace("{{competition}}", competition_str);

	// Follow button
	let is_following = team.followers.contains(&requesting_member.id);
	let star_display = if is_following { "" } else { "none" };
	let star_outline_display = if is_following { "none" } else { "" };
	let page = page.replace("{{star-display}}", star_display);
	let page = page.replace("{{outline-display}}", star_outline_display);

	let status_updates = lock.get_team_status(team.number).await.map_err(|e| {
		error!("Failed to get team status updates from database: {e}");
		Status::InternalServerError
	})?;

	// Status
	let current_status = RobotStatus::get_from_updates(&status_updates);
	let page = page.replace("{{status}}", &current_status.to_string());
	let page = page.replace("{{status-color}}", current_status.get_color());

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
			r#"<div class="cont round comp-cb"><input type=checkbox {disabled_attr} {checked_attr} data-val={comp} /> {}</div>"#,
			comp.get_abbr()
		);
		checkboxes_string.push_str(&component);
	}
	let page = page.replace("{{comp-checkboxes}}", &checkboxes_string);

	let page = page.replace(
		"{{edit-button}}",
		include_str!("../../components/ui/edit.min.html"),
	);

	// Create stats
	let epa = state
		.statbotics_client
		.get_epa(id)
		.await
		.unwrap_or_default();
	let page = page.replace("{{epa}}", &render_stat_card_float("EPA", "", epa, true, ""));

	let default_stats = CombinedTeamStats::default();
	let lock2 = state.team_stats.read().await;
	let team_stats = lock2.get(&id).unwrap_or(&default_stats);
	let page = page.replace(
		"{{apa}}",
		stat_card_float!(team_stats, "APA", apa, "apa", true),
	);
	let page = page.replace(
		"{{win-rate}}",
		stat_card_pct!(team_stats, "Win Rate", win_rate, "win_rate", true),
	);
	let page = page.replace(
		"{{matches}}",
		stat_card_other!(team_stats, "Matches", matches, "matches", false),
	);
	let page = page.replace(
		"{{reliability}}",
		stat_card_pct!(team_stats, "Reliability", reliability, "reliability", false),
	);
	let page = page.replace(
		"{{penalties}}",
		stat_card_other!(team_stats, "Penalties", penalties, "penalties", false),
	);
	let page = page.replace(
		"{{auto-score}}",
		stat_card_float!(team_stats, "Score", auto_score, "auto_score", true),
	);
	let page = page.replace(
		"{{auto-accuracy}}",
		stat_card_pct!(team_stats, "Accuracy", auto_accuracy, "auto_accuracy", true),
	);
	let page = page.replace(
		"{{auto-collisions}}",
		stat_card_other!(
			team_stats,
			"Collisions",
			auto_collisions,
			"auto_collisions",
			false
		),
	);
	let page = page.replace(
		"{{cycle-time}}",
		stat_card_float!(team_stats, "CT", cycle_time, "cycle_time", true),
	);
	let page = page.replace(
		"{{cycle-time-consistency}}",
		stat_card_pct!(
			team_stats,
			"CTC",
			cycle_time_consistency,
			"cycle_time_consistency",
			true
		),
	);
	let page = page.replace(
		"{{speaker-score}}",
		stat_card_float!(
			team_stats,
			"Spkr Sco",
			speaker_score,
			"speaker_score",
			false
		),
	);
	let page = page.replace(
		"{{amp-score}}",
		stat_card_float!(team_stats, "Amp Sco", amp_score, "amp_score", false),
	);
	let page = page.replace(
		"{{cycle-time-deviation}}",
		stat_card_float!(
			team_stats,
			"CTD",
			cycle_time_devation,
			"cycle_time_deviation",
			true
		),
	);
	let page = page.replace(
		"{{pass-average}}",
		stat_card_float!(team_stats, "Pass Avg", pass_average, "pass_average", false),
	);
	let page = page.replace(
		"{{speaker-accuracy}}",
		stat_card_pct!(
			team_stats,
			"Spkr Acc",
			speaker_accuracy,
			"speaker_accuracy",
			false
		),
	);
	let page = page.replace(
		"{{amp-accuracy}}",
		stat_card_pct!(team_stats, "Amp Acc", amp_accuracy, "amp_accuracy", false),
	);
	let page = page.replace(
		"{{amp-rate}}",
		stat_card_float!(
			team_stats,
			"Amp Rate",
			amplification_rate,
			"amplification_rate",
			true
		),
	);
	let page = page.replace(
		"{{amp-power}}",
		stat_card_float!(
			team_stats,
			"Amp Pwr",
			amplification_power,
			"amplification_power",
			true
		),
	);
	let page = page.replace(
		"{{defense-average}}",
		stat_card_float!(
			team_stats,
			"Def Avg",
			defense_average,
			"defense_average",
			false
		),
	);
	let page = page.replace(
		"{{climb-score}}",
		stat_card_float!(team_stats, "Climb Sco", climb_score, "climb_score", true),
	);
	let page = page.replace(
		"{{climb-accuracy}}",
		stat_card_pct!(
			team_stats,
			"Climb Acc",
			climb_accuracy,
			"climb_accuracy",
			false
		),
	);
	let page = page.replace(
		"{{trap-score}}",
		stat_card_float!(team_stats, "Trap Sco", trap_score, "trap_score", true),
	);
	let page = page.replace(
		"{{trap-accuracy}}",
		stat_card_pct!(
			team_stats,
			"Trap Acc",
			trap_accuracy,
			"trap_accuracy",
			false
		),
	);

	// Team info
	let team_info = lock
		.get_team_info(team.number)
		.await
		.map_err(|e| {
			error!("Failed to get team info from database: {e}");
			Status::InternalServerError
		})?
		.unwrap_or_default();

	let page = page.replace(
		"{{max-speed}}",
		&render_stat_card_optional_float("Max Speed", "", team_info.max_speed, true, ""),
	);
	let page = page.replace(
		"{{height}}",
		&render_stat_card_optional_float("Height", "", team_info.height, true, ""),
	);
	let page = page.replace(
		"{{weight}}",
		&render_stat_card_optional_float("Weight", "", team_info.weight, true, ""),
	);
	let page = page.replace(
		"{{length}}",
		&render_stat_card_optional_float("Length", "", team_info.length, false, ""),
	);
	let page = page.replace(
		"{{width}}",
		&render_stat_card_optional_float("Width", "", team_info.width, false, ""),
	);
	let page = page.replace(
		"{{drivetrain-type}}",
		&render_stat_card_optional(
			"Drivetrain",
			"",
			team_info.drivetrain_type.map(|x| match x {
				DriveTrainType::Swerve => "Sw",
				DriveTrainType::Tank => "Tk",
				DriveTrainType::Mecanum => "Mc",
				DriveTrainType::Other => "Ot",
			}),
			false,
			"",
		),
	);
	let page = page.replace(
		"{{can-pickup-algae}}",
		&render_stat_card_optional_bool("Algae Int?", "", team_info.can_pickup_algae, false, ""),
	);
	let page = page.replace(
		"{{can-pickup-coral}}",
		&render_stat_card_optional_bool("Coral Int?", "", team_info.can_pickup_coral, false, ""),
	);
	let page = page.replace(
		"{{can-hold-both}}",
		&render_stat_card_optional_bool("Hold Both?", "", team_info.can_hold_both, false, ""),
	);
	let page = page.replace(
		"{{can-ground-intake}}",
		&render_stat_card_optional_bool("Ground Int?", "", team_info.can_ground_intake, false, ""),
	);
	let page = page.replace(
		"{{can-slide-intake}}",
		&render_stat_card_optional_bool("Slide Int?", "", team_info.can_slide_intake, false, ""),
	);
	let page = page.replace(
		"{{can-reef}}",
		&render_stat_card_optional_bool("Reef?", "", team_info.can_reef, false, ""),
	);
	let page = page.replace(
		"{{can-processor}}",
		&render_stat_card_optional_bool("Processor?", "", team_info.can_processor, false, ""),
	);
	let page = page.replace(
		"{{can-net}}",
		&render_stat_card_optional_bool("Net?", "", team_info.can_net, false, ""),
	);
	let page = page.replace(
		"{{reef-level}}",
		&render_stat_card_optional(
			"Reef Lvl",
			"",
			team_info.reef_level.map(|x| match x {
				ReefLevel::L1 => "L1",
				ReefLevel::L2 => "L2",
				ReefLevel::L3 => "L3",
				ReefLevel::L4 => "L4",
			}),
			true,
			"",
		),
	);
	let page = page.replace(
		"{{climb-ability}}",
		&render_stat_card_optional(
			"Climb",
			"",
			team_info.climb_ability.map(|x| match x {
				ClimbAbility::None => "X",
				ClimbAbility::Shallow => "S",
				ClimbAbility::Deep => "D",
			}),
			true,
			"",
		),
	);
	let page = page.replace(
		"{{preferred-piece}}",
		&render_stat_card_optional(
			"Fave",
			"",
			team_info.preferred_piece.map(|x| match x {
				GamePiece::Algae => "Al",
				GamePiece::Coral => "Co",
			}),
			false,
			"",
		),
	);
	let page = page.replace(
		"{{pit-cycle-time}}",
		&render_stat_card_optional_float("CT", "", team_info.cycle_time, false, ""),
	);
	let page = page.replace(
		"{{climb-time}}",
		&render_stat_card_optional_float("Clmb Time", "", team_info.climb_time, false, ""),
	);
	let page = page.replace(
		"{{align-score}}",
		&render_stat_card_optional_bool("Score Align?", "", team_info.align_score, false, ""),
	);
	let page = page.replace(
		"{{align-intake}}",
		&render_stat_card_optional_bool("Intk Align?", "", team_info.align_intake, false, ""),
	);
	let page = page.replace(
		"{{auto-crosses-line}}",
		&render_stat_card_optional_bool("Auto Cross?", "", team_info.auto_crosses_line, false, ""),
	);
	let page = page.replace(
		"{{auto-scores-front}}",
		&render_stat_card_optional_bool("Auto Front?", "", team_info.auto_scores_front, false, ""),
	);
	let page = page.replace(
		"{{auto-scores-back}}",
		&render_stat_card_optional_bool("Auto Back?", "", team_info.auto_scores_back, false, ""),
	);
	let page = page.replace(
		"{{auto-scores-side}}",
		&render_stat_card_optional_bool("Auto Side?", "", team_info.auto_scores_side, false, ""),
	);
	let page = page.replace(
		"{{auto-algae}}",
		&render_stat_card_optional(
			"Auto Alg",
			"",
			team_info.auto_algae.map(|x| x.to_string()),
			false,
			"",
		),
	);
	let page = page.replace(
		"{{auto-coral}}",
		&render_stat_card_optional(
			"Auto Cor",
			"",
			team_info.auto_coral.map(|x| x.to_string()),
			false,
			"",
		),
	);
	let page = page.replace(
		"{{uses-pathplanner}}",
		&render_stat_card_optional_bool("PP?", "", team_info.uses_pathplanner, false, ""),
	);
	let page = page.replace(
		"{{two-can-networks}}",
		&render_stat_card_optional_bool("2CAN?", "", team_info.two_can_networks, false, ""),
	);

	let page = page.replace("{{notes}}", &team_info.notes);

	// Pit scouting progress
	let page = page.replace("{{pit-scouting-progress}}", &team_info.progress.to_string());
	let page = page.replace("{{pit-scouting-color}}", team_info.progress.get_color());

	let page = create_page("Team Details", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
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
