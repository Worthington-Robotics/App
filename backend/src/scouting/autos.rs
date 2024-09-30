use std::{
	collections::{HashMap, HashSet},
	fmt::Display,
};

use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

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
	/// How often this auto is used
	pub usage_rate: f32,
	/// How long the auto takes in seconds
	pub duration: Option<f32>,
	/// The average time per shot in seconds
	pub time_per_shot: Option<f32>,
	/// The time to the first shot in seconds
	pub time_to_first_shot: Option<f32>,
	/// The max speed of the robot during the auto
	pub max_speed: Option<f32>,
	/// The total distance covered during the auto
	pub distance_travelled: f32,
	/// The detected starting position of the auto
	pub starting_position: StartingPosition,
}

/// Starting point for an auto
#[derive(Clone, Default, Debug, Copy, EnumIter)]
pub enum StartingPosition {
	SpeakerCenter,
	SpeakerAmp,
	SpeakerWall,
	#[default]
	Other,
}

impl StartingPosition {
	/// Get the field coordinate where the position is
	pub fn get_pos(&self) -> Option<(f32, f32)> {
		match self {
			Self::SpeakerAmp => Some((0.66, 6.62)),
			Self::SpeakerCenter => Some((1.37, 5.57)),
			Self::SpeakerWall => Some((0.66, 4.51)),
			Self::Other => None,
		}
	}
}

impl Display for StartingPosition {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::Other => "Other",
				Self::SpeakerAmp => "Spkr Amp",
				Self::SpeakerCenter => "Spkr Center",
				Self::SpeakerWall => "Spkr Wall",
			}
		)
	}
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
		let mut uses = 0;
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

			uses += 1;
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

		let usage_rate = uses as f32 / fix_zero(matches.len() as f32);

		let duration = auto.points.last().map(|x| x.time);
		let time_to_first_shot = auto.shots.first().map(|x| x.time);

		let mut distance_travelled = 0.0;
		let mut max_speed = 0.0;
		for points in auto.points.windows(2) {
			let delta =
				((points[1].x - points[0].x).powi(2) + (points[1].y - points[0].y).powi(2)).sqrt();
			distance_travelled += delta;
			let speed = delta / fix_zero(points[1].time - points[0].time);
			if speed > max_speed {
				max_speed = speed;
			}
		}

		let mut shot_time_sum = 0.0;
		for shots in auto.shots.windows(2) {
			let delta = shots[1].time - shots[0].time;
			shot_time_sum += delta;
		}
		// Include the time from the first shot in the sum
		if let Some(first_time) = time_to_first_shot {
			shot_time_sum += first_time;
		}
		let time_per_shot = shot_time_sum / fix_zero(auto.shots.len() as f32);

		// Detect the starting location
		let starting_position = if let Some(starting_point) = auto.points.first() {
			let starting_pos = (starting_point.x, starting_point.y);
			let closest = StartingPosition::iter()
				.filter_map(|option| {
					if let Some(pos) = option.get_pos() {
						let dist = ((pos.0 - starting_pos.0).powi(2)
							+ (pos.1 - starting_pos.1).powi(2))
						.sqrt();

						// Reject if it is outside of a threshold
						if dist > 1.0 {
							None
						} else {
							Some((dist, option))
						}
					} else {
						None
					}
				})
				.min_by(|x, y| x.0.partial_cmp(&y.0).unwrap())
				.map(|x| x.1);

			closest.unwrap_or_default()
		} else {
			StartingPosition::Other
		};

		let stats = AutoStats {
			average_notes: note_total,
			accuracy: note_total / fix_zero(shot_chances.len() as f32),
			shot_chances,
			usage_rate,
			duration,
			time_to_first_shot,
			distance_travelled,
			max_speed: Some(max_speed),
			time_per_shot: Some(time_per_shot),
			starting_position,
		};

		auto_stats.insert(auto.id, stats);
	}
}
