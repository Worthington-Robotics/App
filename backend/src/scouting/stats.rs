use std::{collections::HashMap, ops::DerefMut, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use rocket::{
	fairing::{Fairing, Info, Kind},
	tokio::sync::RwLock,
	Orbit, Rocket,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{
	db::{Database, DatabaseImpl},
	util::{fix_zero, standard_deviation},
};

use super::{
	autos::{calculate_auto_stats, AutoStats},
	game::{get_coral_points, ClimbAbility, ClimbResult},
	matches::MatchStats,
	status::RobotStatus,
	Competition, TeamNumber,
};

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
	pub fn calculate(
		team: TeamNumber,
		matches: &[MatchStats],
		current_competition: Option<Competition>,
	) -> Self {
		let mut historical = Vec::new();
		for i in 1..matches.len() {
			historical.push(calculate_team_stats(team, &matches[i - 1..i]));
		}

		let all_time = calculate_team_stats(team, &matches);

		let current_competition_stats = if let Some(current_competition) = current_competition {
			let current_competition_matches: Vec<_> = matches
				.into_iter()
				.filter(|x| x.competition.is_some_and(|x| x == current_competition))
				.cloned()
				.collect();

			calculate_team_stats(team, &current_competition_matches)
		} else {
			all_time.clone()
		};

		Self {
			historical,
			current_competition: current_competition_stats,
			all_time,
		}
	}
}

/// Stored and calculated stats for a single team
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct TeamStats {
	pub number: TeamNumber,
	pub epa: f32,
	pub apa: f32,
	pub win_rate: f32,
	pub coral_score: f32,
	pub coral_average: f32,
	pub coral_accuracy: f32,
	pub algae_score: f32,
	pub processor_average: f32,
	pub processor_accuracy: f32,
	pub net_average: f32,
	pub intake_accuracy: f32,
	pub climb_accuracy: f32,
	pub climb_time: f32,
	pub auto_coral: f32,
	pub auto_algae: f32,
	pub auto_coral_accuracy: f32,
	pub auto_algae_accuracy: f32,
	pub auto_collisions: u8,
	/// Average number of offensive moves per match
	pub offense_average: f32,
	/// Average number of defensive moves per match
	pub defense_average: f32,
	/// Average cycle time
	pub cycle_time: f32,
	/// Consistency of cycle time
	pub cycle_time_consistency: f32,
	/// Standard deviation for cycle time
	pub cycle_time_devation: f32,
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
	if matches.is_empty() {
		return TeamStats::default();
	}

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

	let cycle_time_average = ctx.cycle_time_sum as f32 / match_count_f32;
	let cycle_time_devation = standard_deviation(&ctx.cycle_times, cycle_time_average);

	TeamStats {
		number: team,
		epa: 0.0,
		apa: ctx.points_scored as f32 / match_count_f32,
		win_rate: ctx.wins as f32 / match_count_f32,
		coral_score: ctx.coral_score_total as f32 / match_count_f32,
		coral_average: ctx.coral_scores as f32 / match_count_f32,
		coral_accuracy: ctx.coral_scores as f32 / fix_zero(ctx.coral_attempts as f32),
		algae_score: ctx.algae_score_total as f32 / match_count_f32,
		processor_average: ctx.processor_scores as f32 / match_count_f32,
		processor_accuracy: ctx.processor_scores as f32 / fix_zero(ctx.processor_attempts as f32),
		net_average: ctx.net_scores as f32 / match_count_f32,
		climb_accuracy: ctx.climb_successes as f32 / fix_zero(ctx.climb_attempts as f32),
		climb_time: ctx.climb_time_total / fix_zero(ctx.climb_attempts as f32),
		auto_coral: ctx.auto_coral_scores as f32 / match_count_f32,
		auto_coral_accuracy: ctx.auto_coral_scores as f32
			/ fix_zero(ctx.auto_coral_attempts as f32),
		auto_algae: ctx.auto_algae_scores as f32 / match_count_f32,
		auto_algae_accuracy: ctx.auto_algae_scores as f32
			/ fix_zero(ctx.auto_algae_attempts as f32),
		auto_collisions: ctx.auto_collisions,
		offense_average: (ctx.coral_scores as f32
			+ ctx.processor_scores as f32
			+ ctx.net_scores as f32)
			/ match_count_f32,
		defense_average: ctx.defenses as f32 / match_count_f32,
		cycle_time: cycle_time_average,
		cycle_time_consistency: ctx.cycle_time_consistency_sum as f32
			/ fix_zero(ctx.cycle_time_consistency_count as f32),
		cycle_time_devation,
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
	points_scored: u16,
	auto_coral_attempts: u16,
	auto_coral_scores: u16,
	auto_algae_attempts: u16,
	auto_algae_scores: u16,
	auto_collisions: u8,
	coral_attempts: u16,
	coral_scores: u16,
	coral_score_total: u16,
	processor_attempts: u16,
	processor_scores: u16,
	net_scores: u16,
	algae_score_total: u16,
	intake_attempts: u16,
	intake_successes: u16,
	climb_attempts: u16,
	climb_successes: u16,
	climb_time_total: f32,
	defenses: u16,
	penalties: u8,
	cycle_time_sum: f32,
	cycle_time_consistency_sum: f32,
	/// Total number of matches where cycle time consistency was added to the sum
	cycle_time_consistency_count: u16,
	/// All cycle times
	cycle_times: Vec<f32>,
	breaks: u8,
	/// Total number of times the team showed up for the match
	attendance: u16,
	wins: u16,
}

/// Add stats from a match to running stat totals in the context
fn process_match(stats: &MatchStats, ctx: &mut StatsContext) {
	ctx.total_matches += 1;
	ctx.auto_coral_attempts += stats.auto_coral_attempts.len() as u16;
	ctx.auto_coral_scores += stats
		.auto_coral_attempts
		.iter()
		.filter(|x| x.successful)
		.count() as u16;
	ctx.auto_algae_attempts += stats.auto_algae_attempts as u16;
	ctx.auto_algae_scores += stats.auto_algae_scores as u16;

	if stats.auto_collision {
		ctx.auto_collisions += 1;
	}

	ctx.points_scored += stats.points_scored as u16;
	ctx.coral_attempts += stats.teleop_coral_attempts.len() as u16;
	ctx.coral_scores += stats
		.teleop_coral_attempts
		.iter()
		.filter(|x| x.successful)
		.count() as u16;
	ctx.processor_attempts += stats.processor_attempts as u16;
	ctx.processor_scores += stats.processor_scores as u16;
	ctx.net_scores += stats.net_shots as u16;
	ctx.intake_attempts += stats.teleop_intake_attempts as u16;
	ctx.intake_successes += stats.teleop_intake_successes as u16;

	let mut coral_score_total = 0;
	for attempt in &stats.teleop_coral_attempts {
		if attempt.successful {
			coral_score_total += get_coral_points(attempt.level, false) as u16;
		}
	}
	ctx.coral_score_total += coral_score_total;

	ctx.algae_score_total += stats.processor_scores as u16 * 6;
	ctx.algae_score_total += stats.net_shots as u16 * 4;

	if stats.climb_attempted != ClimbAbility::None {
		ctx.climb_attempts += 1;
	}
	if stats.climb_result == ClimbResult::Succeeded {
		ctx.climb_successes += 1;
		ctx.climb_time_total += stats.climb_time;
	}

	ctx.defenses += stats.defenses as u16;
	ctx.penalties += stats.penalties;

	ctx.cycle_time_sum += stats.cycle_time;
	if let Some(consistency) = calculate_cycle_consistency(&stats.cycle_times) {
		ctx.cycle_time_consistency_sum += consistency;
		ctx.cycle_time_consistency_count += 1;
	}

	// Get the deltas between the cycle timestamps
	let mut cycle_deltas = Vec::with_capacity(stats.cycle_times.len());
	for window in stats.cycle_times.windows(2) {
		cycle_deltas.push(window[1] - window[0]);
	}

	ctx.cycle_times.extend(cycle_deltas);

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
	db: Arc<RwLock<DatabaseImpl>>,
	team_stats: Arc<RwLock<HashMap<TeamNumber, CombinedTeamStats>>>,
	auto_stats: Arc<RwLock<HashMap<String, AutoStats>>>,
}

impl UpdateStats {
	pub fn new(
		db: Arc<RwLock<DatabaseImpl>>,
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
				let lock = db.read().await;

				let global_data = match lock.get_global_data().await {
					Ok(global_data) => global_data,
					Err(e) => {
						error!("Failed to get global data from database: {e}");
						return;
					}
				};

				let match_stats =
					match lock.get_all_match_stats().await {
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

				let teams = match lock.get_teams().await {
					Ok(teams) => teams,
					Err(e) => {
						error!("Failed to update stats: Failed to get teams from database: {e}");
						return;
					}
				};

				let teams: Vec<_> = teams.map(|x| x.number).collect();

				let mut stats = HashMap::with_capacity(teams.len());
				for team in &teams {
					let team_stats = CombinedTeamStats::calculate(
						*team,
						&match_stats,
						global_data.current_competition,
					);
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
				std::mem::drop(auto_stats);

				std::mem::drop(lock);

				rocket::tokio::time::sleep(Duration::from_secs(30)).await;
			}
		});
	}
}

/// Calculate the consistency of cycle times by getting the r^2 value of the linear regression of the times.
/// Returns None if there are no cycle times
fn calculate_cycle_consistency(cycle_times: &[f32]) -> Option<f32> {
	if cycle_times.len() < 2 {
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
