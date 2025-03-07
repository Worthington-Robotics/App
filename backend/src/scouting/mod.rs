pub mod assignment;
pub mod autos;
/// Utilities for the game, such as point calculations
pub mod game;
pub mod matches;
pub mod stats;
/// Robot broken status tracking
pub mod status;

use std::{collections::HashSet, fmt::Display};

use chrono_tz::{
	Tz,
	US::{Central, Eastern},
};
use game::{ClimbAbility, GamePiece, ReefLevel};
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
		let out = self.name.replace(|x: char| !x.is_ascii(), "");
		if out.is_empty() {
			self.name.clone()
		} else {
			out
		}
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
	Week1,
	Week2,
	Week3,
	Week4,
	Week5,
	Week6,
}

impl Competition {
	pub fn from_db(val: &str) -> Option<Self> {
		match val {
			"Pittsburgh" => Some(Self::Pittsburgh),
			"Buckeye" => Some(Self::Buckeye),
			"MiamiValley" => Some(Self::MiamiValley),
			"Champs" => Some(Self::Champs),
			"States" => Some(Self::States),
			"Week1" => Some(Self::Week1),
			"Week2" => Some(Self::Week2),
			"Week3" => Some(Self::Week3),
			"Week4" => Some(Self::Week4),
			"Week5" => Some(Self::Week5),
			"Week6" => Some(Self::Week6),
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
			Self::Week1 => "Wk1",
			Self::Week2 => "Wk2",
			Self::Week3 => "Wk3",
			Self::Week4 => "Wk4",
			Self::Week5 => "Wk5",
			Self::Week6 => "Wk6",
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
			Self::Week1 | Self::Week2 | Self::Week3 | Self::Week4 | Self::Week5 | Self::Week6 => {
				None
			}
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
	pub team: TeamNumber,
	/// The max speed of the robot, in feet per second
	pub max_speed: Option<f32>,
	/// The height of the robot, in inches, when fully extended
	pub height: Option<f32>,
	/// The weight of the robot, in pounds
	pub weight: Option<f32>,
	/// The length of the robot, from front to back, in inches
	pub length: Option<f32>,
	/// The width of the robot, from left to right, in inches
	pub width: Option<f32>,
	/// The drivetrain type of the robot
	pub drivetrain_type: Option<DriveTrainType>,
	pub can_pickup_algae: Option<bool>,
	pub can_pickup_coral: Option<bool>,
	pub can_hold_both: Option<bool>,
	pub can_ground_intake_algae: Option<bool>,
	pub can_ground_intake_coral: Option<bool>,
	pub can_slide_intake: Option<bool>,
	pub can_reef: Option<bool>,
	pub can_processor: Option<bool>,
	pub can_net: Option<bool>,
	pub can_agitate: Option<bool>,
	pub can_l1: Option<bool>,
	pub can_l2: Option<bool>,
	pub can_l3: Option<bool>,
	pub can_l4: Option<bool>,
	pub can_shallow: Option<bool>,
	pub can_deep: Option<bool>,
	pub preferred_piece: Option<GamePiece>,
	pub cycle_time: Option<f32>,
	pub climb_time: Option<f32>,
	pub align_score: Option<bool>,
	pub align_intake: Option<bool>,
	pub auto_crosses_line: Option<bool>,
	pub auto_scores_front: Option<bool>,
	pub auto_scores_back: Option<bool>,
	pub auto_scores_side: Option<bool>,
	pub auto_algae: Option<u8>,
	pub auto_coral: Option<u8>,
	pub uses_pathplanner: Option<bool>,
	pub two_can_networks: Option<bool>,
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
