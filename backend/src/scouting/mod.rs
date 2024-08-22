pub mod matches;

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter, IntoStaticStr};

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
#[derive(Display, EnumIter, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, IntoStaticStr)]
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
#[derive(Serialize, Deserialize)]
pub struct TeamStats {
	pub number: TeamNumber,
	pub epa: f32,
	pub apa: f32,
	pub win_rate: f32,
	pub speaker_accuracy: f32,
	pub amp_accuracy: f32,
	pub climb_accuracy: f32,
	pub trap_accuracy: f32,
	/// Average number of notes scored per auto
	pub auto_score: f32,
	/// Average number of amplifications per match
	pub amplification_rate: f32,
	/// Average number of notes per amplification
	pub amplification_value: f32,
	/// Average number of passes per match
	pub pass_rate: f32,
	/// Average number of offensive moves per match
	pub offense_rate: f32,
	/// Average number of defensive moves per match
	pub defense_rate: f32,
	/// Total number of penalties
	pub penalties: u16,
	/// Rate that the team shows up to the match with a working robot (0-1)
	pub availablity: f32,
}

/// Scouting assignments for a member
#[derive(Serialize, Deserialize)]
pub struct ScoutingAssignments {
	pub member: String,
	pub teams: HashSet<TeamNumber>,
}
