use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::util::{fix_zero, vector_splat};

use super::{game::get_coral_points, matches::MatchStats, TeamNumber};

/// A single autonomous routine
#[derive(Deserialize, Serialize, Clone)]
pub struct Auto {
	/// The ID of the auto
	pub id: String,
	/// The name of the auto
	pub name: String,
	/// The team this auto is for
	pub team: TeamNumber,
	/// How many coral this auto attempts to score
	pub coral: u8,
	/// How many algae this auto attempts to score
	pub algae: u8,
	/// Whether this auto attempts to agitate algae
	pub agitates: bool,
	/// The position of the auto on the starting line, in meters
	pub starting_position: f32,
}

/// Stats for a single auto
#[derive(Clone, Default, Debug)]
pub struct AutoStats {
	/// The average point value of this auto
	pub point_value: f32,
	/// Average number of coral that this auto gets
	pub average_coral: f32,
	/// Average number of algae that this auto gets
	pub average_algae: f32,
	/// Average coral accuracy for this auto
	pub coral_accuracy: f32,
	/// Average algae accuracy for this auto
	pub algae_accuracy: f32,
	/// Chances out of 1 for each coral placement of the auto to be successful, in order
	pub coral_chances: Vec<f32>,
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
		let mut coral_hits = vector_splat(0u16, auto.coral as usize);
		let mut coral_misses = vector_splat(0u16, auto.coral as usize);
		let mut algae_attempts = 0.0;
		let mut algae_hits = 0.0;
		let mut coral_points = 0.0;
		let mut uses = 0;
		for m in matches {
			if m.team_number != team {
				continue;
			}
			if !m.auto.as_ref().is_some_and(|x| x == &auto.id) {
				continue;
			}

			for (i, shot) in m.auto_coral_attempts.iter().enumerate() {
				if i >= auto.coral as usize {
					break;
				}

				if shot.successful {
					coral_hits[i] += 1;
					coral_points += get_coral_points(shot.level, true) as f32;
				} else {
					coral_misses[i] += 1;
				}
			}

			algae_attempts += m.auto_algae_attempts as f32;
			algae_hits += m.auto_algae_scores as f32;

			uses += 1;
		}

		let mut coral_total = 0.0;
		let mut coral_chances = Vec::with_capacity(auto.coral as usize);
		for (hits, misses) in coral_hits.into_iter().zip(coral_misses) {
			let chance = if hits + misses == 0 {
				0.0
			} else {
				hits as f32 / (hits + misses) as f32
			};
			coral_total += chance;
			coral_chances.push(chance);
		}

		let usage_rate = uses as f32 / fix_zero(matches.len() as f32);
		let fixed_use_total = fix_zero(uses as f32);

		let mut point_value = 0.0;
		point_value += coral_points;
		point_value += algae_hits as f32 * 6.0;
		point_value /= fixed_use_total;

		let stats = AutoStats {
			average_coral: coral_total / fixed_use_total,
			average_algae: algae_hits / fixed_use_total,
			coral_accuracy: coral_total / fix_zero(coral_chances.len() as f32),
			coral_chances,
			algae_accuracy: algae_hits / fix_zero(algae_attempts),
			usage_rate,
			point_value,
		};

		auto_stats.insert(auto.id, stats);
	}
}
