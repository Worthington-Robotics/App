pub mod assignment;
pub mod autos;
pub mod matches;
pub mod status;

use std::{
	collections::{HashMap, HashSet},
	ops::DerefMut,
	sync::Arc,
	time::Duration,
};

use autos::{calculate_auto_stats, AutoStats};
use chrono::{DateTime, Utc};
use chrono_tz::{
	Tz,
	US::{Central, Eastern},
};
use matches::MatchStats;
use rocket::{
	fairing::{Fairing, Info, Kind},
	tokio::sync::{Mutex, RwLock},
	Orbit, Rocket,
};
use serde::{Deserialize, Serialize};
use status::RobotStatus;
use strum_macros::{Display, EnumIter, IntoStaticStr};
use tracing::error;

use crate::{
	db::{Database, DatabaseImpl},
	util::fix_zero,
};

/// Type for the number of a team
pub type TeamNumber = u16;

/// A single team
#[derive(Serialize, Deserialize, Clone)]
pub struct Team {
	pub number: TeamNumber,
	pub name: String,
	pub rookie_year: i32,
	pub competitions: HashSet<Competition>,
	#[serde(default)]
	pub followers: HashSet<String>,
}

impl Team {
	/// Get this team's sanitized name with things like emojis removed
	pub fn sanitized_name(&self) -> String {
		self.name.replace(|x: char| !x.is_ascii(), "")
	}
}

/// Competition that the team will attend
#[derive(
	Display, EnumIter, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, IntoStaticStr,
)]
#[serde(rename_all = "snake_case")]
pub enum Competition {
	Pittsburgh,
	Buckeye,
	MiamiValley,
	Champs,
	States,
}

impl Competition {
	pub fn from_db(val: &str) -> Option<Self> {
		match val {
			"Pittsburgh" => Some(Self::Pittsburgh),
			"Buckeye" => Some(Self::Buckeye),
			"MiamiValley" => Some(Self::MiamiValley),
			"Champs" => Some(Self::Champs),
			"States" => Some(Self::States),
			_ => None,
		}
	}

	pub fn get_abbr(&self) -> &'static str {
		match self {
			Self::Pittsburgh => "GPR",
			Self::Buckeye => "BR",
			Self::MiamiValley => "MVR",
			Self::Champs => "CMPTX",
			Self::States => "OSC",
		}
	}

	/// Gets the FRC event code of this event
	pub fn get_code(&self) -> Option<&'static str> {
		match self {
			Self::Pittsburgh => Some("PACA"),
			Self::Buckeye => Some("OHCL"),
			Self::MiamiValley => Some("OHMV"),
			Self::Champs => None,
			Self::States => None,
		}
	}

	/// Gets the timezone of this event
	pub fn get_timezone(&self) -> Tz {
		match self {
			Self::Champs => Central,
			_ => Eastern,
		}
	}
}

/// Information about a team and their robot, mostly obtained from pit scouting
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct TeamInfo {
	/// The max speed of the robot, in feet per second
	pub max_speed: Option<f32>,
	/// The height of the robot, in feet
	pub height: Option<f32>,
	/// The weight of the robot, in pounds
	pub weight: Option<f32>,
	/// The length of the robot, from front to back, in feet
	pub length: Option<f32>,
	/// The width of the robot, from left to right, in feet
	pub width: Option<f32>,
	/// Whether or not the robot can shoot in the speaker
	pub can_speaker: Option<bool>,
	/// Whether or not the robot can shoot in the amp
	pub can_amp: Option<bool>,
	/// Whether or not the robot can climb
	pub can_climb: Option<bool>,
	/// Whether or not the robot can shoot in the trap
	pub can_trap: Option<bool>,
	/// Whether or not the robot can pass notes
	pub can_pass: Option<bool>,
	/// Whether or not the robot can drive under the stage
	pub can_drive_under_stage: Option<bool>,
	/// Whether or not the robot can pick up from the ground
	pub can_ground_intake: Option<bool>,
	/// Whether or not the robot can pick up from the source
	pub can_source_intake: Option<bool>,
	/// The intake type of the robot
	pub intake_type: Option<IntakeType>,
	/// The drivetrain type of the robot
	pub drivetrain_type: Option<DriveTrainType>,
	/// Additional notes about the robot
	pub notes: String,
}

/// Different types of intakes
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntakeType {
	OverBumper,
	UnderBumper,
}

/// Different types of drivetrains
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DriveTrainType {
	Swerve,
	Tank,
	Mecanum,
	Other,
}

/// Combination of all-time and historical stats for a single team
#[derive(Default)]
pub struct CombinedTeamStats {
	pub historical: Vec<TeamStats>,
	pub current_competition: TeamStats,
	pub all_time: TeamStats,
}

impl CombinedTeamStats {
	/// Calculate combined team stats. The input matches don't all have to be from this team, but they do have
	/// to be in date order
	pub fn calculate(team: TeamNumber, matches: &[MatchStats]) -> Self {
		let mut historical = Vec::new();
		for i in 1..matches.len() {
			historical.push(calculate_team_stats(team, &matches[0..i]));
		}

		let all_time = calculate_team_stats(team, &matches);

		// TODO: Use actual competition
		let current_competition: Vec<_> = matches
			.into_iter()
			.filter(|x| x.competition.is_some_and(|x| x == Competition::Pittsburgh))
			.cloned()
			.collect();
		let current_competition = calculate_team_stats(team, &current_competition);

		Self {
			historical,
			current_competition,
			all_time,
		}
	}
}

/// Stored and calculated stats for a single team
#[derive(Serialize, Deserialize, Default)]
pub struct TeamStats {
	pub number: TeamNumber,
	pub epa: f32,
	pub apa: f32,
	pub win_rate: f32,
	pub speaker_score: f32,
	pub speaker_accuracy: f32,
	pub amp_score: f32,
	pub amp_accuracy: f32,
	pub climb_score: f32,
	pub climb_accuracy: f32,
	pub trap_score: f32,
	pub trap_accuracy: f32,
	/// Average number of notes scored per auto
	pub auto_score: f32,
	pub auto_collisions: u8,
	pub auto_accuracy: f32,
	/// Average number of amplifications per match
	pub amplification_rate: f32,
	/// Average number of notes per amplification
	pub amplification_power: f32,
	/// Average number of passes per match
	pub pass_average: f32,
	/// Average number of offensive moves per match
	pub offense_average: f32,
	/// Average number of defensive moves per match
	pub defense_average: f32,
	/// Average cycle time
	pub cycle_time: f32,
	/// Consistency of cycle time
	pub cycle_time_consistency: f32,
	/// Total number of penalties
	pub penalties: u8,
	/// Rate that the team shows up to the match with a working robot (0-1)
	pub reliability: f32,
	/// Total number of matches the team has played
	pub matches: u16,
}

/// Calculate stats for a single team. The given set of stats can contain matches from other teams,
/// and the correct ones will automatically be filtered through
pub fn calculate_team_stats(team: TeamNumber, matches: &[MatchStats]) -> TeamStats {
	let mut ctx = StatsContext::default();
	for m in matches {
		if m.team_number != team {
			continue;
		}

		process_match(m, &mut ctx);
	}

	let mut match_count_f32 = ctx.total_matches as f32;
	// Account for all div by zero cases by just setting the denominator to 1
	if match_count_f32 == 0.0 {
		match_count_f32 = 1.0;
	}

	TeamStats {
		number: team,
		epa: 0.0,
		apa: ctx.points_scored as f32 / match_count_f32,
		win_rate: ctx.wins as f32 / match_count_f32,
		speaker_score: ctx.speaker_scores as f32 / match_count_f32,
		speaker_accuracy: ctx.speaker_scores as f32 / fix_zero(ctx.speaker_attempts as f32),
		amp_score: ctx.amp_scores as f32 / match_count_f32,
		amp_accuracy: ctx.amp_scores as f32 / fix_zero(ctx.amp_attempts as f32),
		climb_score: ctx.climb_successes as f32 / match_count_f32,
		climb_accuracy: ctx.climb_successes as f32 / fix_zero(ctx.climb_attempts as f32),
		trap_score: ctx.trap_successes as f32 / match_count_f32,
		trap_accuracy: ctx.trap_successes as f32 / fix_zero(ctx.trap_attempts as f32),
		auto_score: ctx.auto_scores as f32 / match_count_f32,
		auto_collisions: ctx.auto_collisions,
		auto_accuracy: ctx.auto_scores as f32 / fix_zero(ctx.auto_attempts as f32),
		amplification_rate: ctx.amplifications as f32 / match_count_f32,
		amplification_power: ctx.amplified_notes as f32 / match_count_f32,
		pass_average: ctx.passes as f32 / match_count_f32,
		offense_average: (ctx.amp_scores as f32 + ctx.speaker_scores as f32) / match_count_f32,
		defense_average: ctx.defenses as f32 / match_count_f32,
		cycle_time: ctx.cycle_time_sum as f32 / match_count_f32,
		cycle_time_consistency: ctx.cycle_time_consistency_sum as f32
			/ fix_zero(ctx.cycle_time_consistency_count as f32),
		penalties: ctx.penalties,
		reliability: (ctx.attendance - ctx.breaks as u16) as f32 / match_count_f32,
		matches: ctx.total_matches as u16,
		..Default::default()
	}
}

/// Context for calculating stats that is updated as match stats are read to do things like sum totals
#[derive(Default)]
struct StatsContext {
	total_matches: u16,
	auto_attempts: u16,
	auto_scores: u16,
	auto_collisions: u8,
	points_scored: u16,
	amp_attempts: u16,
	amp_scores: u16,
	speaker_attempts: u16,
	speaker_scores: u16,
	climb_attempts: u16,
	climb_successes: u16,
	trap_attempts: u16,
	trap_successes: u16,
	amplifications: u16,
	amplified_notes: u16,
	passes: u16,
	defenses: u16,
	penalties: u8,
	cycle_time_sum: f32,
	cycle_time_consistency_sum: f32,
	/// Total number of matches where cycle time consistency was added to the sum
	cycle_time_consistency_count: u16,
	breaks: u8,
	/// Total number of times the team showed up for the match
	attendance: u16,
	wins: u16,
}

/// Add stats from a match to running stat totals in the context
fn process_match(stats: &MatchStats, ctx: &mut StatsContext) {
	ctx.total_matches += 1;
	ctx.auto_attempts += stats.auto_attempts as u16;
	ctx.auto_scores += stats.auto_scores as u16;
	if stats.auto_collision {
		ctx.auto_collisions += 1;
	}
	ctx.points_scored += stats.points_scored as u16;
	ctx.amp_attempts += stats.amp_attempts as u16;
	ctx.amp_scores += stats.amp_scores as u16;
	ctx.speaker_attempts += stats.speaker_attempts as u16;
	ctx.speaker_scores += stats.speaker_scores as u16;
	if stats.climb_attempted {
		ctx.climb_attempts += 1;
	}
	if stats.climb_successful {
		ctx.climb_successes += 1;
	}
	if stats.trap_attempted {
		ctx.trap_attempts += 1;
	}
	if stats.trap_successful {
		ctx.trap_successes += 1;
	}
	ctx.amplifications += stats.amplifications as u16;
	ctx.amplified_notes += stats.amplified_notes as u16;
	ctx.passes += stats.passes as u16;
	ctx.defenses += stats.defenses as u16;
	ctx.penalties += stats.penalties;

	ctx.cycle_time_sum += stats.cycle_time;
	if let Some(consistency) = calculate_cycle_consistency(&stats.cycle_times) {
		ctx.cycle_time_consistency_sum += consistency;
		ctx.cycle_time_consistency_count += 1;
	}

	if stats.status != RobotStatus::Good {
		ctx.breaks += 1;
	}
	if stats.showed_up {
		ctx.attendance += 1;
	}
	if stats.won {
		ctx.wins += 1;
	}
}

/// Fairing for periodically updating team stats
pub struct UpdateStats {
	db: Arc<Mutex<DatabaseImpl>>,
	team_stats: Arc<RwLock<HashMap<TeamNumber, CombinedTeamStats>>>,
	auto_stats: Arc<RwLock<HashMap<String, AutoStats>>>,
}

impl UpdateStats {
	pub fn new(
		db: Arc<Mutex<DatabaseImpl>>,
		team_stats: Arc<RwLock<HashMap<TeamNumber, CombinedTeamStats>>>,
		auto_stats: Arc<RwLock<HashMap<String, AutoStats>>>,
	) -> Self {
		Self {
			db,
			team_stats,
			auto_stats,
		}
	}
}

#[async_trait::async_trait]
impl Fairing for UpdateStats {
	fn info(&self) -> Info {
		Info {
			name: "Update Stats",
			kind: Kind::Liftoff,
		}
	}

	async fn on_liftoff(&self, _: &Rocket<Orbit>) {
		// Periodically update stats
		let db = self.db.clone();
		let stored_stats = self.team_stats.clone();
		let stored_auto_stats = self.auto_stats.clone();
		rocket::tokio::spawn(async move {
			loop {
				// In a scope so that locks aren't held while waiting for the next loop
				{
					let lock = db.lock().await;
					let match_stats = match lock.get_all_match_stats().await {
						Ok(stats) => stats,
						Err(e) => {
							error!("Failed to update stats: Failed to get match stats from database: {e}");
							return;
						}
					};
					let mut match_stats: Vec<_> = match_stats.collect();
					match_stats.sort_by_cached_key(|x| {
						let Some(record_time) = &x.record_time else {
							return Utc::now();
						};

						let Ok(date) = DateTime::parse_from_rfc2822(&record_time) else {
							return Utc::now();
						};

						date.to_utc()
					});

					let teams =
						match lock.get_teams().await {
							Ok(teams) => teams,
							Err(e) => {
								error!("Failed to update stats: Failed to get teams from database: {e}");
								return;
							}
						};

					let teams: Vec<_> = teams.map(|x| x.number).collect();

					let mut stats = HashMap::with_capacity(teams.len());
					for team in &teams {
						let team_stats = CombinedTeamStats::calculate(*team, &match_stats);
						stats.insert(*team, team_stats);
					}

					*stored_stats.write().await = stats;

					let mut auto_stats = stored_auto_stats.write().await;
					for team in teams {
						let autos = match lock.get_autos(team).await {
							Ok(autos) => autos,
							Err(e) => {
								error!("Failed to update stats: Failed to get autos for team from database: {e}");
								return;
							}
						};

						calculate_auto_stats(team, &match_stats, autos, auto_stats.deref_mut());
					}
				}

				rocket::tokio::time::sleep(Duration::from_secs(30)).await;
			}
		});
	}
}

/// Calculate the consistency of cycle times by getting the r^2 value of the linear regression of the times.
/// Returns None if there are no cycle times
fn calculate_cycle_consistency(cycle_times: &[f32]) -> Option<f32> {
	if cycle_times.is_empty() {
		return None;
	}

	let x_mean = cycle_times.iter().sum::<f32>() / cycle_times.len() as f32;
	// All of the y-values will just be a linear sequence of integers, so the mean is the number of y-values / 2
	let y_mean = cycle_times.len() as f32 / 2.0;

	// First calculate the a coefficient of the regression y = ax + b
	let mut numerator = 0.0;
	let mut denominator = 0.0;
	for (i, time) in cycle_times.into_iter().enumerate() {
		let x = *time;
		let y = i as f32;
		numerator += (x - x_mean) * (y - y_mean);
		denominator += (x - x_mean).powi(2);
	}
	let a = numerator / denominator;

	// Now calculate b
	let b = y_mean - (a * x_mean);

	/* Calculate the sum of the residuals (deltas of actual values from the regression) each squared,
		along with the total sum of squares, which is the y deltas from the y mean each squared
	*/
	let mut ssr = 0.0;
	let mut sst = 0.0;
	for (i, time) in cycle_times.into_iter().enumerate() {
		let x = *time;
		let y = i as f32;
		let expected_y = a * x + b;
		let delta = y - expected_y;
		ssr += delta * delta;

		let delta = y - y_mean;
		sst += delta * delta;
	}

	let r_2 = 1.0 - (ssr / sst);

	Some(r_2)
}
