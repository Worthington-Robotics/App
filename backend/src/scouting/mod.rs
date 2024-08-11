pub mod matches;

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// Type for the number of a team
pub type TeamNumber = u16;

/// A single team
#[derive(Serialize, Deserialize)]
pub struct Team {
	pub number: TeamNumber,
	pub name: String,
}

/// Information about a team's robot, mostly obtained from pit scouting
#[derive(Serialize, Deserialize)]
pub struct RobotInfo {
	pub number: TeamNumber,
	/// The max speed of the robot, in feet per second
	pub max_speed: f32,
	/// The height of the robot, in feet
	pub height: f32,
	/// The weight of the robot, in pounds
	pub weight: f32,
}

/// Scouting assignments for a member
#[derive(Serialize, Deserialize)]
pub struct ScoutingAssignments {
	pub member: String,
	pub teams: HashSet<TeamNumber>,
}
