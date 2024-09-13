use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::util::{fix_zero, vector_splat};

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
	/// List of points for the auto, with timestamps
	pub points: Vec<AutoPoint>,
	/// List of points where the auto shoots a note
	pub shots: Vec<AutoPoint>,
	/// Notes that the auto attempts to take. They are numbered like this:
	///
	/// 3    8
	/// 2    7
	/// 1    6
	///      5
	///      4
	pub notes: HashSet<u8>,
}

/// A single point of an auto, with an x, y, and timestamp
#[derive(Deserialize, Serialize, Clone)]
pub struct AutoPoint {
	pub x: f32,
	pub y: f32,
	pub time: f32,
}

impl AutoPoint {
	/// Construct a list of AutoPoint structs from multiple lists of each field
	pub fn list_from_fields(x_list: &[f32], y_list: &[f32], time_list: &[f32]) -> Vec<Self> {
		debug_assert_eq!(x_list.len(), y_list.len());
		debug_assert_eq!(x_list.len(), time_list.len());

		x_list
			.into_iter()
			.zip(y_list)
			.zip(time_list)
			.map(|((x, y), time)| Self {
				x: *x,
				y: *y,
				time: *time,
			})
			.collect()
	}

	/// Construct multiple lists of each field from a list of AutoPoints
	pub fn list_to_fields(points: &[Self]) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
		let mut x_points = Vec::with_capacity(points.len());
		let mut y_points = Vec::with_capacity(points.len());
		let mut time_points = Vec::with_capacity(points.len());
		for point in points {
			x_points.push(point.x);
			y_points.push(point.y);
			time_points.push(point.time);
		}

		(x_points, y_points, time_points)
	}
}

/// Stats for a single auto
#[derive(Clone, Default, Debug)]
pub struct AutoStats {
	/// Average number of notes that this auto gets
	pub average_notes: f32,
	/// Average accuracy for this auto
	pub accuracy: f32,
	/// Chances out of 1 for each shot of the auto to hit, in order
	pub shot_chances: Vec<f32>,
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
		let mut shot_hits = vector_splat(0u16, auto.shots.len());
		let mut shot_misses = vector_splat(0u16, auto.shots.len());
		for m in matches {
			if m.team_number != team {
				continue;
			}
			if !m.auto.as_ref().is_some_and(|x| x == &auto.id) {
				continue;
			}

			for (i, shot) in m.auto_shots.iter().enumerate() {
				if *shot {
					shot_hits[i] += 1;
				} else {
					shot_misses[i] += 1;
				}
			}
		}

		let mut note_total = 0.0;
		let mut shot_chances = Vec::with_capacity(auto.shots.len());
		for (hits, misses) in shot_hits.into_iter().zip(shot_misses) {
			let chance = if hits + misses == 0 {
				0.0
			} else {
				hits as f32 / (hits + misses) as f32
			};
			note_total += chance;
			shot_chances.push(chance);
		}

		let stats = AutoStats {
			average_notes: note_total,
			accuracy: note_total / fix_zero(shot_chances.len() as f32),
			shot_chances,
		};

		auto_stats.insert(auto.id, stats);
	}
}
