pub mod matches;

use std::{
	collections::{HashMap, HashSet},
	sync::Arc,
	time::Duration,
};

use matches::MatchStats;
use rocket::{
	fairing::{Fairing, Info, Kind},
	tokio::sync::{Mutex, RwLock},
	Orbit, Rocket,
};
use serde::{Deserialize, Serialize};
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
}

/// Information about a team's robot, mostly obtained from pit scouting
#[derive(Serialize, Deserialize, Clone)]
pub struct RobotInfo {
	pub number: TeamNumber,
	/// The max speed of the robot, in feet per second
	pub max_speed: f32,
	/// The height of the robot, in feet
	pub height: f32,
	/// The weight of the robot, in pounds
	pub weight: f32,
	/// Whether or not the robot can shoot in the speaker
	pub can_speaker: bool,
	/// Whether or not the robot can shoot in the amp
	pub can_amp: bool,
	/// Whether or not the robot can climb
	pub can_climb: bool,
	/// Whether or not the robot can shoot in the trap
	pub can_trap: bool,
	/// Whether or not the robot can pass notes
	pub can_pass: bool,
	/// Whether or not the robot can drive under the stage
	pub can_drive_under_stage: bool,
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
	pub availablity: f32,
	/// Total number of matches the team has played
	pub matches: u16,
}

/// Scouting assignments for a member
#[derive(Serialize, Deserialize)]
pub struct ScoutingAssignments {
	pub member: String,
	pub teams: HashSet<TeamNumber>,
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
		availablity: (ctx.attendance - ctx.breaks) as f32 / match_count_f32,
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
	attendance: u8,
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

	if stats.broken {
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
	team_stats: Arc<RwLock<HashMap<TeamNumber, TeamStats>>>,
}

impl UpdateStats {
	pub fn new(
		db: Arc<Mutex<DatabaseImpl>>,
		team_stats: Arc<RwLock<HashMap<TeamNumber, TeamStats>>>,
	) -> Self {
		Self { db, team_stats }
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

					let match_stats: Vec<_> = match_stats.collect();
					let teams =
						match lock.get_teams().await {
							Ok(teams) => teams,
							Err(e) => {
								error!("Failed to update stats: Failed to get teams from database: {e}");
								return;
							}
						};

					let teams = teams.map(|x| x.number);

					let mut stats = HashMap::with_capacity(teams.size_hint().0);
					for team in teams {
						let team_stats = calculate_team_stats(team, &match_stats);
						stats.insert(team, team_stats);
					}

					*stored_stats.write().await = stats;
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
