use serde::{Deserialize, Serialize};

use super::TeamNumber;

/// A single match
#[derive(Serialize, Deserialize, Clone)]
pub struct Match {
	pub id: String,
	pub red_alliance: Vec<TeamNumber>,
	pub blue_alliance: Vec<TeamNumber>,
}

/// Stats for a single team in a match
#[derive(Serialize, Deserialize, Clone)]
pub struct MatchStats {
	/// The team that got these stats
	pub team_number: TeamNumber,
	/// The match where these stats occurred
	pub match_id: String,
	/// The number of times that the team attempted to score during auto
	pub auto_attempts: u8,
	/// The number of times that the team scored during auto
	pub auto_scores: u8,
	/// The number of auto intake attempts
	#[serde(default)]
	pub auto_intake_attempts: u8,
	/// The number of auto intake successes
	#[serde(default)]
	pub auto_intake_successes: u8,
	/// Whether or not the robot collided with another during auto
	pub auto_collision: bool,
	/// The total number of points that the team scored
	pub points_scored: u16,
	/// The number of times that the team attempted to score in the amp
	pub amp_attempts: u8,
	/// The number of times that the team scored in the amp
	pub amp_scores: u8,
	/// The number of times that the team attempted to score in the speaker
	pub speaker_attempts: u8,
	/// The number of times that the team scored in the speaker
	pub speaker_scores: u8,
	/// Whether or not the team attempted to climb
	pub climb_attempted: bool,
	/// Whether or not the team succeeded the climb
	pub climb_successful: bool,
	/// Whether or not the team attempted the trap
	pub trap_attempted: bool,
	/// Whether or not the team succeeded the trap
	pub trap_successful: bool,
	/// The number of times that the alliance amplified
	pub amplifications: u8,
	/// The number of times that the team scored into an amplified speaker
	pub amplified_notes: u8,
	/// The number of times that the team passed notes
	pub passes: u8,
	/// The number of times that the team defended against other robots
	pub defenses: u8,
	/// The number of penalties that the team incurred during the match
	pub penalties: u8,
	/// The team's average cycle time
	#[serde(default)]
	pub cycle_time: f32,
	/// The team's individual cycle timestamps
	#[serde(default)]
	pub cycle_times: Vec<f32>,
	/// Whether the robot was reported as broken
	pub broken: bool,
	/// Whether or not the team showed up to the match
	pub showed_up: bool,
	/// Whether or not the team won the match
	#[serde(default)]
	pub won: bool,
	/// Additional notes about the match
	pub notes: String,
}
