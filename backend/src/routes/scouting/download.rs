use std::io::Cursor;

use itertools::Itertools;
use rocket::{http::Status, Responder};
use serde::Serialize;
use tracing::{error, span, Level};

use crate::{
	db::Database,
	routes::OptionalSessionID,
	scouting::{
		game::{ClimbAbility, ClimbResult},
		stats::CombinedTeamStats,
		status::RobotStatus,
		Competition, TeamNumber,
	},
	State,
};

#[rocket::get("/api/scouting_download/team_stats.csv")]
pub async fn download_team_stats(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<Downloadable, Status> {
	let span = span!(Level::DEBUG, "Downloading team stats");
	let _enter = span.enter();

	let Some(session_id) = session_id.to_session_id() else {
		return Err(Status::Unauthorized);
	};

	if session_id.verify_elevated(state).await.is_err() {
		return Err(Status::Unauthorized);
	};

	let out = team_stats_to_csv(state, false).await?;

	Ok(Downloadable(out))
}

#[rocket::get("/api/scouting_download/team_stats_current_competition.csv")]
pub async fn download_team_stats_current_comp(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<Downloadable, Status> {
	let span = span!(
		Level::DEBUG,
		"Downloading team stats from current competition"
	);
	let _enter = span.enter();

	let Some(session_id) = session_id.to_session_id() else {
		return Err(Status::Unauthorized);
	};

	if session_id.verify_elevated(state).await.is_err() {
		return Err(Status::Unauthorized);
	};

	let out = team_stats_to_csv(state, true).await?;

	Ok(Downloadable(out))
}

pub async fn team_stats_to_csv(state: &State, current_comp: bool) -> Result<Vec<u8>, Status> {
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

		let stats = if current_comp {
			&stats.current_competition
		} else {
			&stats.all_time
		};

		if let Err(e) = csv_writer.serialize(stats) {
			error!("Failed to serialize row: {e}");
			continue;
		};
	}
	if let Err(e) = csv_writer.flush() {
		error!("Failed to flush CSV buffer: {e}");
		return Err(Status::InternalServerError);
	}

	std::mem::drop(csv_writer);

	Ok(buf)
}

#[rocket::get("/api/scouting_download/matches.csv")]
pub async fn download_matches(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<Downloadable, Status> {
	let span = span!(Level::DEBUG, "Downloading matches");
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

	let mut buf = Vec::new();
	let mut csv_writer = csv::Writer::from_writer(Cursor::new(&mut buf));

	for m in match_stats {
		let mut auto_fuel_attempts = 0;
		let mut auto_fuel_scores = 0;
		for volley in &m.auto_fuel_volleys {
			auto_fuel_attempts += volley.shots_attempted as u16;
			auto_fuel_scores += volley.shots_made as u16;
		}

		let mut teleop_fuel_attempts = 0;
		let mut teleop_fuel_scores = 0;
		for volley in &m.teleop_fuel_volleys {
			teleop_fuel_attempts += volley.shots_attempted as u16;
			teleop_fuel_scores += volley.shots_made as u16;
		}

		let mut teleop_intakes = 0;
		for volley in &m.teleop_intake_volleys {
			teleop_intakes += volley.shots_made as u16;
		}

		let mut teleop_passes = 0;
		for volley in &m.teleop_pass_volleys {
			teleop_passes += volley.shots_made as u16;
		}

		let cycle_times = format!(
			"[{}]",
			m.cycle_times.into_iter().map(|x| x.to_string()).join(",")
		);

		let record = CSVMatchStats {
			team_number: m.team_number,
			match_id: m.match_id,
			match_number: m.match_number.map(|x| x.to_string()),
			recorder: m.recorder,
			record_time: m.record_time,
			recorded_live: m.recorded_live,
			competition: m.competition,
			auto: m.auto,
			auto_fuel_attempts,
			auto_fuel_scores,
			auto_fuel_volleys: m.auto_fuel_volleys.len() as u16,
			auto_climb_attempted: m.auto_climb_attempted,
			auto_climb_successful: m.auto_climb_successful,
			auto_collision: m.auto_collision,
			teleop_fuel_attempts,
			teleop_fuel_scores,
			teleop_fuel_volleys: m.teleop_fuel_volleys.len() as u16,
			teleop_intakes,
			teleop_intake_volleys: m.teleop_intake_volleys.len() as u16,
			teleop_passes,
			teleop_pass_volleys: m.teleop_pass_volleys.len() as u16,
			climb_attempted: m.climb_attempted,
			climb_result: m.climb_result,
			climb_time: m.climb_time,
			points_scored: m.points_scored,
			defenses: m.defenses,
			penalties: m.penalties,
			cycle_time: Some(m.cycle_time).filter(|x| x != &0.0),
			cycle_times,
			status: m.status,
			showed_up: m.showed_up,
			won: m.won,
			notes: m.notes,
			strengths: m.strengths,
			weaknesses: m.weaknesses,
			brownout: m.brownout,
			tipping: m.tipping,
			beached: m.beached,
			teleop_pause: m.teleop_pause,
			game_piece_stuck: m.game_piece_stuck,
		};

		if let Err(e) = csv_writer.serialize(&record) {
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

/// Match stats that can be serialized to CSV
#[derive(Serialize)]
pub struct CSVMatchStats {
	/// The team that got these stats
	pub team_number: TeamNumber,
	/// The match where these stats occurred
	pub match_id: String,
	/// The match number for these stats
	#[serde(default)]
	pub match_number: Option<String>,
	/// The member who recorded these stats
	#[serde(default)]
	pub recorder: Option<String>,
	/// When the stats were recorded, as a DateTime
	#[serde(default)]
	pub record_time: Option<String>,
	/// Whether this match happened live when it was recorded
	#[serde(default)]
	pub recorded_live: bool,
	/// The competition that this match is associated with
	#[serde(default)]
	pub competition: Option<Competition>,
	/// The auto that the team ran during this match
	#[serde(default)]
	pub auto: Option<String>,
	pub auto_fuel_attempts: u16,
	pub auto_fuel_scores: u16,
	pub auto_fuel_volleys: u16,
	/// Whether or not an auto climb was attempted
	pub auto_climb_attempted: bool,
	/// WHether or not an auto climb was successful
	pub auto_climb_successful: bool,
	/// Whether or not the robot collided with another during auto
	pub auto_collision: bool,
	pub teleop_fuel_attempts: u16,
	pub teleop_fuel_scores: u16,
	pub teleop_fuel_volleys: u16,
	/// The fuel intake attempts during teleop
	pub teleop_intakes: u16,
	pub teleop_intake_volleys: u16,
	/// The pass attempts during teleop
	pub teleop_passes: u16,
	pub teleop_pass_volleys: u16,
	/// The climb that the team attempted to do
	pub climb_attempted: ClimbAbility,
	/// The result of the climb
	pub climb_result: ClimbResult,
	/// How long the climb took
	pub climb_time: f32,
	/// The total number of points that the team scored
	pub points_scored: i16,
	/// The number of times that the team defended against other robots
	pub defenses: u8,
	/// The number of penalties that the team incurred during the match
	pub penalties: u8,
	/// The team's average cycle time
	#[serde(default)]
	pub cycle_time: Option<f32>,
	/// The team's individual cycle timestamps
	#[serde(default)]
	pub cycle_times: String,
	/// The broken status of the robot
	#[serde(default)]
	pub status: RobotStatus,
	/// Whether or not the team showed up to the match
	pub showed_up: bool,
	/// Whether or not the team won the match
	#[serde(default)]
	pub won: bool,
	/// Additional notes about the match
	pub notes: String,
	/// Team strengths during the match
	#[serde(default)]
	pub strengths: String,
	/// Team weaknesses during the match
	#[serde(default)]
	pub weaknesses: String,
	/// Whether the robot had brownout issues
	pub brownout: bool,
	/// Whether the robot had tipping issues
	pub tipping: bool,
	/// Whether the robot was beached on an algae
	pub beached: bool,
	/// Whether the robot had a large pause when teleop started
	pub teleop_pause: bool,
	/// Whether the robot had a stuck game piece
	pub game_piece_stuck: bool,
}

#[rocket::get("/api/scouting_download/team_info.csv")]
pub async fn download_team_info(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<Downloadable, Status> {
	let span = span!(Level::DEBUG, "Downloading team info");
	let _enter = span.enter();

	let Some(session_id) = session_id.to_session_id() else {
		return Err(Status::Unauthorized);
	};

	if session_id.verify_elevated(state).await.is_err() {
		return Err(Status::Unauthorized);
	};

	let lock = state.db.read().await;
	let team_infos = lock.get_all_team_info().await.map_err(|e| {
		error!("Failed to get team info from database: {e}");
		Status::InternalServerError
	})?;

	let mut buf = Vec::new();
	let mut csv_writer = csv::Writer::from_writer(Cursor::new(&mut buf));

	for info in team_infos {
		if let Err(e) = csv_writer.serialize(&info) {
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
pub struct Downloadable(pub Vec<u8>);
