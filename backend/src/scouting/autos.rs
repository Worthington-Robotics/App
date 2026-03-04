use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::util::{fix_zero, float_max};

use super::{matches::MatchStats, TeamNumber};

/// A single autonomous routine
#[derive(Deserialize, Serialize, Clone)]
pub struct Auto {
	/// The ID of the auto
	pub id: String,
	/// The name of the auto
	pub name: String,
	/// The team this auto is for
	pub team: TeamNumber,
	/// How many fuel this auto attempts to score
	pub fuel: u8,
	/// The position of the auto on the starting line, in meters
	pub starting_position: f32,
}

/// Stats for a single auto
#[derive(Clone, Default, Debug)]
pub struct AutoStats {
	/// The average point value of this auto
	pub point_value: f32,
	/// Average number of fuel that this auto gets
	pub average_fuel: f32,
	/// Average fuel accuracy for this auto
	pub fuel_accuracy: f32,
	/// How often this auto is used
	pub usage_rate: f32,
}

/// Calculate stats for all autos for a single team. The given set of stats can contain matches from other teams,
/// and the correct ones will automatically be filtered through
pub fn calculate_auto_stats(
	team: TeamNumber,
	matches: &[MatchStats],
	autos: impl Iterator<Item = Auto>,
	auto_stats: &mut HashMap<String, AutoStats>,
) {
	for auto in autos {
		let mut fuel_hits = 0.0;
		let mut fuel_misses = 0.0;
		let mut uses = 0;
		let mut total_matches = 0;
		for m in matches {
			if m.team_number != team {
				continue;
			}
			total_matches += 1;
			if !m.auto.as_ref().is_some_and(|x| x == &auto.id) {
				continue;
			}

			for volley in &m.auto_fuel_volleys {
				fuel_hits += volley.shots_made as f32;
				if volley.shots_attempted > volley.shots_made {
					fuel_misses += (volley.shots_attempted - volley.shots_made) as f32;
				}
			}

			uses += 1;
		}

		let fixed_use_total = fix_zero(uses as f32);
		let point_value = fuel_hits / fixed_use_total;

		let usage_rate = uses as f32 / fix_zero(total_matches as f32);

		let stats = AutoStats {
			average_fuel: fuel_hits / fixed_use_total,
			fuel_accuracy: fuel_hits / fix_zero(fuel_hits + fuel_misses),
			usage_rate,
			point_value,
		};

		auto_stats.insert(auto.id, stats);
	}
}

/// How many steps are in the auto event graph, out of 15 seconds
const EVENT_GRAPH_RESOLUTION: usize = 30;
/// How much time to remove from remove from auto events to offset user latency
const EVENT_TIME_OFFSET: f32 = 0.5;

pub fn get_auto_event_graphs(matches: &[MatchStats]) -> AutoEventGraphs {
	if matches.is_empty() {
		return AutoEventGraphs::default();
	}

	let mut shot_times = Vec::new();

	fn clamp_timestamp(timestamp: f32) -> f32 {
		if timestamp < 0.75 {
			0.75
		} else if timestamp > 15.0 {
			15.0 - EVENT_TIME_OFFSET
		} else {
			timestamp - EVENT_TIME_OFFSET
		}
	}

	for m in matches {
		for e in &m.auto_fuel_volleys {
			if e.start_time == 0.0 {
				continue;
			}
			let timestamp = clamp_timestamp(e.start_time);
			shot_times.push(timestamp);
		}
	}

	let mut shots = [0.0; EVENT_GRAPH_RESOLUTION];

	fn get_graph_height(i: usize, times: &[f32]) -> f32 {
		if times.is_empty() {
			return 0.0;
		}

		let dx = 15.0 / EVENT_GRAPH_RESOLUTION as f32;
		let center = i as f32 * dx;
		let left_bound = center - dx;
		let right_bound = center + dx;

		let count = times
			.iter()
			.filter(|x| **x >= left_bound && **x <= right_bound)
			.count();

		let out = count as f32 / times.len() as f32;
		if out > 1.0 {
			1.0
		} else {
			out
		}
	}

	for i in 0..EVENT_GRAPH_RESOLUTION {
		shots[i] = get_graph_height(i, &shot_times);
	}

	// Rescale the graphs so that they are more distinct
	let shot_max = fix_zero(float_max(shots.iter().copied()).unwrap_or(1.0));

	for val in shots.iter_mut() {
		*val /= shot_max;
	}

	AutoEventGraphs { shots }
}

#[derive(Clone)]
pub struct AutoEventGraphs {
	pub shots: [f32; EVENT_GRAPH_RESOLUTION],
}

impl Default for AutoEventGraphs {
	fn default() -> Self {
		Self {
			shots: [0.0; EVENT_GRAPH_RESOLUTION],
		}
	}
}
