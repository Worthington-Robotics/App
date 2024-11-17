pub mod assignment;
pub mod autos;
pub mod matches;
pub mod stats;
pub mod status;

use std::{collections::HashSet, fmt::Display};

use chrono_tz::{
	Tz,
	US::{Central, Eastern},
};
use rocket::FromFormField;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter, IntoStaticStr};

use crate::util::ToDropdown;

/// Type for the number of a team
pub type TeamNumber = u16;

/// A single team
#[derive(Serialize, Deserialize, Clone)]
pub struct Team {
	pub number: TeamNumber,
	pub name: String,
	pub rookie_year: i32,
	pub competitions: HashSet<Competition>,
	#[serde(default)]
	pub followers: HashSet<String>,
}

impl Team {
	/// Get this team's sanitized name with things like emojis removed
	pub fn sanitized_name(&self) -> String {
		self.name.replace(|x: char| !x.is_ascii(), "")
	}
}

/// Competition that the team will attend
#[derive(
	Display,
	EnumIter,
	Copy,
	Clone,
	PartialEq,
	Eq,
	Hash,
	Serialize,
	Deserialize,
	IntoStaticStr,
	FromFormField,
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

	/// Gets the FRC event code of this event
	pub fn get_code(&self) -> Option<&'static str> {
		match self {
			Self::Pittsburgh => Some("PACA"),
			Self::Buckeye => Some("OHCL"),
			Self::MiamiValley => Some("OHMV"),
			Self::Champs => None,
			Self::States => None,
		}
	}

	/// Gets the timezone of this event
	pub fn get_timezone(&self) -> Tz {
		match self {
			Self::Champs => Central,
			_ => Eastern,
		}
	}
}

impl ToDropdown for Competition {
	fn to_dropdown(&self) -> &'static str {
		self.into()
	}
}

/// A FIRST Championship division
#[derive(
	Display,
	EnumIter,
	Copy,
	Clone,
	PartialEq,
	Eq,
	Hash,
	Serialize,
	Deserialize,
	IntoStaticStr,
	FromFormField,
)]
#[serde(rename_all = "snake_case")]
pub enum Division {
	Hopper,
	Newton,
	Galileo,
	Daly,
	Archimedes,
	Curie,
	Johnson,
	Milstein,
}

impl Division {
	pub fn from_db(val: &str) -> Option<Self> {
		match val {
			"Hopper" => Some(Self::Hopper),
			"Newton" => Some(Self::Newton),
			"Galileo" => Some(Self::Galileo),
			"Daly" => Some(Self::Daly),
			"Archimedes" => Some(Self::Archimedes),
			"Curie" => Some(Self::Curie),
			"Johnson" => Some(Self::Johnson),
			"Milstein" => Some(Self::Milstein),
			_ => None,
		}
	}

	/// Gets the FRC event code of this event
	pub fn get_code(&self) -> &'static str {
		match self {
			Self::Hopper => "HOPPER",
			Self::Newton => "NEWTON",
			Self::Galileo => "GALILEO",
			Self::Daly => "DALY",
			Self::Archimedes => "ARCHIMEDES",
			Self::Curie => "CURIE",
			Self::Johnson => "JOHNSON",
			Self::Milstein => "MILSTEIN",
		}
	}
}

impl ToDropdown for Division {
	fn to_dropdown(&self) -> &'static str {
		self.into()
	}
}

/// Information about a team and their robot, mostly obtained from pit scouting
#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(default)]
pub struct TeamInfo {
	/// The max speed of the robot, in feet per second
	pub max_speed: Option<f32>,
	/// The height of the robot, in feet
	pub height: Option<f32>,
	/// The weight of the robot, in pounds
	pub weight: Option<f32>,
	/// The length of the robot, from front to back, in feet
	pub length: Option<f32>,
	/// The width of the robot, from left to right, in feet
	pub width: Option<f32>,
	/// Whether or not the robot can shoot in the speaker
	pub can_speaker: Option<bool>,
	/// Whether or not the robot can shoot in the amp
	pub can_amp: Option<bool>,
	/// Whether or not the robot can climb
	pub can_climb: Option<bool>,
	/// Whether or not the robot can shoot in the trap
	pub can_trap: Option<bool>,
	/// Whether or not the robot can pass notes
	pub can_pass: Option<bool>,
	/// Whether or not the robot can drive under the stage
	pub can_drive_under_stage: Option<bool>,
	/// Whether or not the robot can pick up from the ground
	pub can_ground_intake: Option<bool>,
	/// Whether or not the robot can pick up from the source
	pub can_source_intake: Option<bool>,
	/// The intake type of the robot
	pub intake_type: Option<IntakeType>,
	/// The drivetrain type of the robot
	pub drivetrain_type: Option<DriveTrainType>,
	/// Additional notes about the robot
	pub notes: String,
	/// Completion status of the scouting
	pub progress: PitScoutingProgress,
}

/// Different types of intakes
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IntakeType {
	OverBumper,
	UnderBumper,
}

/// Different types of drivetrains
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DriveTrainType {
	Swerve,
	Tank,
	Mecanum,
	Other,
}

/// Completion status of pit scouting for a team
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PitScoutingProgress {
	#[default]
	NotDone,
	NeedsRefresh,
	Finished,
}

impl Display for PitScoutingProgress {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::NotDone => "Not Done",
				Self::NeedsRefresh => "Needs Refresh",
				Self::Finished => "Finished",
			}
		)
	}
}

impl PitScoutingProgress {
	/// Get the CSS color for this progress
	pub fn get_color(&self) -> &'static str {
		match self {
			Self::NotDone => "var(--wbred)",
			Self::NeedsRefresh => "#eb7134",
			Self::Finished => "#5cd12a",
		}
	}
}
