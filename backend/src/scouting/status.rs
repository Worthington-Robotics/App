use std::{fmt::Display, str::FromStr};

use rocket::FromFormField;
use serde::{Deserialize, Serialize};

use super::{Competition, TeamNumber};

/// Status update (broken / fixed) for a team
#[derive(Serialize, Deserialize, Clone)]
pub struct StatusUpdate {
	pub team: TeamNumber,
	pub date: String,
	pub status: RobotStatus,
	pub details: String,
	pub member: String,
	pub competition: Option<Competition>,
}

impl StatusUpdate {
	/// Infer status reasons from update details
	pub fn infer_reasons(&self) -> Vec<StatusReason> {
		let mut out = Vec::new();

		let lowercase = self.details.to_lowercase();

		if lowercase.contains("intake") {
			out.push(StatusReason::Intake);
		}
		if lowercase.contains("drive")
			|| lowercase.contains("swerve")
			|| lowercase.contains("driving")
		{
			out.push(StatusReason::Drivetrain);
		}
		if lowercase.contains("shoot") {
			out.push(StatusReason::Shooter);
		}
		if lowercase.contains("climb") {
			out.push(StatusReason::Climber);
		}
		if lowercase.contains("hit")
			|| lowercase.contains("collide")
			|| lowercase.contains("impact")
			|| lowercase.contains("strike")
		{
			out.push(StatusReason::Hit);
		}
		if lowercase.contains("disconnect") || lowercase.contains("connect") {
			out.push(StatusReason::Disconnect);
		}
		if lowercase.contains("disable") || lowercase.contains("turn off") {
			out.push(StatusReason::Disabled);
		}
		if lowercase.contains("battery") {
			out.push(StatusReason::Battery);
		}

		out
	}
}

/// Status type for a robot
#[derive(Serialize, Deserialize, Clone, Copy, FromFormField, PartialEq, Eq, Default)]
pub enum RobotStatus {
	#[default]
	Good,
	#[serde(alias = "Injured")]
	Questionable,
	Broken,
}

impl Display for RobotStatus {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::Good => "Good",
				Self::Questionable => "Questionable",
				Self::Broken => "Broken",
			}
		)
	}
}

impl FromStr for RobotStatus {
	type Err = ();
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"Good" => Ok(Self::Good),
			"Injured" | "Questionable" => Ok(Self::Questionable),
			"Broken" => Ok(Self::Broken),
			_ => Err(()),
		}
	}
}

impl RobotStatus {
	/// Get the abbreviated form of this status
	pub fn get_abbr(&self) -> &'static str {
		match self {
			Self::Good => "G",
			Self::Questionable => "Q",
			Self::Broken => "B",
		}
	}

	/// Get the CSS color for this status
	pub fn get_color(&self) -> &'static str {
		match self {
			Self::Good => "#5cd12a",
			Self::Questionable => "#eb7134",
			Self::Broken => "var(--wbred)",
		}
	}

	pub fn to_db(&self) -> &'static str {
		match self {
			Self::Good => "Good",
			Self::Questionable => "Questionable",
			Self::Broken => "Broken",
		}
	}

	/// Get the status of a robot from a list of status updates in chronological order
	pub fn get_from_updates(updates: &[StatusUpdate]) -> Self {
		if updates.is_empty() {
			return Self::Good;
		}

		updates.last().expect("Should not be empty").status
	}
}

/// Reasons / keywords from status updates that are detected from the details
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StatusReason {
	Intake,
	Drivetrain,
	Shooter,
	Climber,
	Elevator,
	Hit,
	Disconnect,
	Disabled,
	Battery,
}

impl Display for StatusReason {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::Intake => "Intake",
				Self::Drivetrain => "Drive",
				Self::Shooter => "Shooter",
				Self::Climber => "Climber",
				Self::Elevator => "Elevator",
				Self::Hit => "Hit",
				Self::Disconnect => "Disconnect",
				Self::Disabled => "Disabled",
				Self::Battery => "Battery",
			}
		)
	}
}
