use std::{collections::HashMap, ops::DerefMut, sync::Arc, time::Duration};

use chrono::{DateTime, Utc};
use rocket::{
	fairing::{Fairing, Info, Kind},
	tokio::sync::RwLock,
	Orbit, Rocket,
};
use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use tracing::error;

use crate::{
	db::{Database, DatabaseImpl},
	scouting::matches::EventVolley,
	util::{fix_zero, standard_deviation},
};

use super::{
	autos::{calculate_auto_stats, AutoStats},
	game::{ClimbAbility, ClimbResult},
	matches::MatchStats,
	status::RobotStatus,
	Competition, TeamNumber,
};

/// Combination of all-time and historical stats for a single team
#[derive(Default)]
pub struct CombinedTeamStats {
	pub historical: Vec<TeamStats>,
	pub current_competition: TeamStats,
	pub per_competition: HashMap<Competition, TeamStats>,
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

		let matches: Vec<_> = matches
			.into_iter()
			.filter(|x| x.team_number == team)
			.cloned()
			.collect();
		// Calculate the moving average
		for i in 1..matches.len() {
			let start = (i as isize - 3).max(0) as usize;
			historical.push(calculate_team_stats(team, &matches[start..i]));
		}

		let all_time = calculate_team_stats(team, &matches);

		let current_competition_stats = if let Some(current_competition) = current_competition {
			let current_competition_matches: Vec<_> = matches
				.iter()
				.filter(|x| x.competition.is_some_and(|x| x == current_competition))
				.cloned()
				.collect();

			calculate_team_stats(team, &current_competition_matches)
		} else {
			all_time.clone()
		};

		let mut per_competition = HashMap::new();
		for competition in Competition::iter() {
			let matches: Vec<_> = matches
				.iter()
				.filter(|x| x.competition == Some(competition))
				.cloned()
				.collect();
			per_competition.insert(competition, calculate_team_stats(team, &matches));
		}

		Self {
			historical,
			current_competition: current_competition_stats,
			per_competition,
			all_time,
		}
	}
}

/// Stored and calculated stats for a single team
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct TeamStats {
	// General
	pub number: TeamNumber,
	pub win_rate: f32,
	// Points
	pub epa: f32,
	pub apa: f32,
	// RP
	pub ranking_points: f32,
	pub fuel_rp: f32,
	pub climb_rp: f32,
	// Teleop
	pub teleop_score: f32,
	pub active_efficiency: f32,
	pub inactive_efficiency: f32,
	// Fuel
	pub fuel_score: f32,
	pub fuel_accuracy: f32,
	pub fuel_speed: f32,
	pub fuel_per_volley: f32,
	// Intake
	pub intake_speed: f32,
	pub fuel_per_intake: f32,
	// Pass
	pub pass_average: f32,
	pub fuel_per_pass: f32,
	// Climb
	pub climb_accuracy: f32,
	pub climb_time: f32,
	pub climb_fall_percent: f32,
	pub climb_score: f32,
	// Auto
	pub auto_fuel: f32,
	pub auto_fuel_accuracy: f32,
	pub auto_climb_accuracy: f32,
	pub auto_collisions: u8,
	pub auto_score: f32,
	/// Average cycle time
	pub cycle_time: f32,
	/// Consistency of cycle time
	pub cycle_time_consistency: f32,
	/// Standard deviation for cycle time
	pub cycle_time_deviation: f32,
	/// Total number of points scored
	pub total_points: i32,
	/// Total number of fuel scored
	pub total_fuel: u32,
	/// Highest scoring match
	pub high_score: i16,
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

	let match_count_f32 = fix_zero(ctx.total_matches as f32);

	let cycle_time_average = ctx.cycle_time_sum as f32 / fix_zero(ctx.cycle_time_count as f32);
	let cycle_time_deviation = standard_deviation(&ctx.cycle_times, cycle_time_average);

	let reliability = if ctx.breaks as u16 >= ctx.attendance {
		0.0
	} else {
		(ctx.attendance - ctx.breaks as u16) as f32 / match_count_f32
	};

	// NOTICE: Update when (not if) we get to champs
	let fuel_rp_avg = calculate_fuel_rp(ctx.fuel_scores as f32 / match_count_f32);
	let climb_rp_avg = ctx.climb_score_total as f32 / 50.0 / match_count_f32;

	let auto_score_total = ctx.auto_score_total;
	let teleop_score_total = ctx.teleop_score_total;
	let climb_score_total = ctx.climb_score_total;
	let points_scored = auto_score_total + teleop_score_total + climb_score_total as i32
		- (ctx.penalties as i32 * 4);

	TeamStats {
		number: team,
		win_rate: ctx.wins as f32 / match_count_f32,
		epa: 0.0,
		apa: points_scored as f32 / match_count_f32,
		ranking_points: fuel_rp_avg + climb_rp_avg,
		fuel_rp: fuel_rp_avg,
		climb_rp: climb_rp_avg,
		teleop_score: teleop_score_total as f32 / match_count_f32,
		active_efficiency: ctx.active_efficiency_total / match_count_f32,
		inactive_efficiency: ctx.inactive_efficiency_total / match_count_f32,
		fuel_score: ctx.fuel_scores as f32 / match_count_f32,
		fuel_accuracy: ctx.fuel_scores as f32 / fix_zero(ctx.fuel_attempts as f32),
		fuel_speed: ctx.fuel_speed_total / fix_zero(ctx.timeable_fuel_volleys as f32),
		fuel_per_volley: ctx.fuel_attempts as f32 / fix_zero(ctx.fuel_volleys as f32),
		intake_speed: ctx.intake_speed_total / fix_zero(ctx.timeable_intake_volleys as f32),
		fuel_per_intake: ctx.intakes as f32 / fix_zero(ctx.intake_volleys as f32),
		pass_average: ctx.passes as f32 / match_count_f32,
		fuel_per_pass: ctx.passes as f32 / fix_zero(ctx.pass_volleys as f32),
		climb_accuracy: ctx.climb_successes as f32 / fix_zero(ctx.climb_attempts as f32),
		climb_time: ctx.climb_time_total / fix_zero(ctx.climb_successes as f32),
		climb_fall_percent: ctx.climb_falls as f32 / fix_zero(ctx.climb_attempts as f32),
		climb_score: climb_score_total as f32 / match_count_f32,
		auto_fuel: ctx.auto_fuel_scores as f32 / match_count_f32,
		auto_fuel_accuracy: ctx.auto_fuel_scores as f32 / fix_zero(ctx.auto_fuel_attempts as f32),
		auto_climb_accuracy: ctx.auto_climb_successes as f32 / fix_zero(ctx.auto_climb_attempts as f32),
		auto_collisions: ctx.auto_collisions,
		auto_score: auto_score_total as f32 / match_count_f32,
		cycle_time: cycle_time_average,
		cycle_time_consistency: ctx.cycle_time_consistency_sum as f32
			/ fix_zero(ctx.cycle_time_consistency_count as f32),
		cycle_time_deviation,
		penalties: ctx.penalties,
		reliability,
		matches: ctx.total_matches as u16,
		total_points: points_scored,
		total_fuel: ctx.fuel_scores + ctx.auto_fuel_scores,
		high_score: ctx.high_score,
		..Default::default()
	}
}

/// Context for calculating stats that is updated as match stats are read to do things like sum totals
#[derive(Default)]
struct StatsContext {
	total_matches: u16,
	high_score: i16,
	auto_fuel_attempts: u32,
	auto_fuel_scores: u32,
	auto_climb_attempts: u8,
	auto_climb_successes: u8,
	auto_collisions: u8,
	auto_score_total: i32,
	fuel_attempts: u32,
	fuel_scores: u32,
	fuel_volleys: u16,
	timeable_fuel_volleys: u16,
	fuel_speed_total: f32,
	intakes: u32,
	intake_volleys: u16,
	timeable_intake_volleys: u16,
	passes: u32,
	pass_volleys: u16,
	intake_speed_total: f32,
	active_efficiency_total: f32,
	inactive_efficiency_total: f32,
	teleop_score_total: i32,
	climb_attempts: u16,
	climb_successes: u16,
	climb_time_total: f32,
	climb_falls: u8,
	climb_score_total: u16,
	defenses: u16,
	penalties: u8,
	cycle_time_sum: f32,
	cycle_time_count: u8,
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
	let mut auto_score = 0;
	let mut teleop_score = 0;
	let mut climb_score = 0;

	ctx.total_matches += 1;

	// Auto

	for volley in &stats.auto_fuel_volleys {
		ctx.auto_fuel_attempts += volley.shots_attempted as u32;
		ctx.auto_fuel_scores += volley.shots_made as u32;
		auto_score += volley.shots_made as i32;
	}

	if stats.auto_climb_attempted {
		ctx.auto_climb_attempts += 1;
		if stats.auto_climb_successful {
			ctx.auto_climb_successes += 1;
			auto_score += 15;
		}
	}

	if stats.auto_collision {
		ctx.auto_collisions += 1;
		auto_score -= 3;
	}

	// Teleop

	for volley in &stats.teleop_fuel_volleys {
		ctx.fuel_volleys += 1;
		ctx.fuel_attempts += volley.shots_attempted as u32;
		ctx.fuel_scores += volley.shots_made as u32;
		teleop_score += volley.shots_made as i32;
		if let Some(speed) = volley.get_rate() {
			ctx.fuel_speed_total += speed;
			ctx.timeable_fuel_volleys += 1;
		}
	}

	for volley in &stats.teleop_intake_volleys {
		ctx.intake_volleys += 1;
		ctx.intakes += volley.shots_made as u32;
		if let Some(speed) = volley.get_rate() {
			ctx.intake_speed_total += speed;
			ctx.timeable_intake_volleys += 1;
		}
	}

	for volley in &stats.teleop_pass_volleys {
		ctx.pass_volleys += 1;
		ctx.passes += volley.shots_made as u32;
	}

	ctx.active_efficiency_total += calculate_shift_efficiency(&stats.teleop_fuel_volleys);
	ctx.inactive_efficiency_total += calculate_shift_efficiency(&stats.teleop_intake_volleys);

	// Climb

	if stats.climb_attempted != ClimbAbility::None {
		ctx.climb_attempts += 1;

		if stats.climb_result == ClimbResult::Succeeded && stats.climb_time > 0.0 {
			ctx.climb_successes += 1;
			ctx.climb_time_total += stats.climb_time;
			climb_score += stats.climb_attempted.get_score();
		} else if stats.climb_result == ClimbResult::Fell {
			ctx.climb_falls += 1;
		}
	}

	ctx.defenses += stats.defenses as u16;
	ctx.penalties += stats.penalties;

	if stats.cycle_time > 1.5 {
		ctx.cycle_time_sum += stats.cycle_time;
		ctx.cycle_time_count += 1;
	}

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

	let total_score = auto_score + teleop_score + climb_score as i32;
	if total_score > ctx.high_score as i32 {
		ctx.high_score = total_score as i16;
	}
	ctx.auto_score_total += auto_score;
	ctx.teleop_score_total += teleop_score;
	ctx.climb_score_total += climb_score as u16;
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

				rocket::tokio::time::sleep(Duration::from_secs(20)).await;
			}
		});
	}
}

/// Calculate the average amount of each shift that a team spends doing some volley
fn calculate_shift_efficiency(volleys: &[EventVolley]) -> f32 {
	let shift_time = 25.0;
	let start = 20.0;

	let mut sum = 0.0;
	let mut count = 0;

	// Calculate transition shift
	if let Some(transition_usage) = calculate_single_shift_usage(start, start + 10.0, volleys) {
		sum += transition_usage;
		count += 1;
	}

	// Calculate normal shifts
	let start = start + 10.0;
	for i in 0..4 {
		let start = start + i as f32 * shift_time;
		let end = start + shift_time;

		if let Some(usage) = calculate_single_shift_usage(start, end, volleys) {
			sum += usage;
			count += 1;
		}
	}

	sum / fix_zero(count as f32)
}

fn calculate_single_shift_usage(
	start_time: f32,
	end_time: f32,
	volleys: &[EventVolley],
) -> Option<f32> {
	if volleys.is_empty() {
		return None;
	}

	let mut sum = 0.0;
	for volley in volleys {
		if volley.start_time < start_time || volley.end_time > end_time {
			continue;
		}

		let Some(duration) = volley.duration() else {
			continue;
		};

		sum += duration;
	}

	if sum == 0.0 {
		None
	} else {
		Some(sum / (end_time - start_time))
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

	if r_2.is_nan() {
		None
	} else {
		Some(r_2)
	}
}

/// Calculates 
fn calculate_fuel_rp(fuel_avg: f32) -> f32 {
	let energized = 100.0;
	let supercharged = 360.0;

	let energized_avg = fuel_avg.clamp(0.0, energized) / energized;
	let supercharged_avg = (fuel_avg - energized).clamp(0.0, f32::INFINITY) / (supercharged - energized);

	energized_avg + supercharged_avg
}
