use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};

use super::{game::ClimbResult, status::RobotStatus, ClimbAbility, Competition, TeamNumber};

/// A single match
#[derive(Serialize, Deserialize, Clone)]
pub struct Match {
	pub num: MatchNumber,
	#[serde(default)]
	pub date: Option<String>,
	pub red_alliance: Vec<TeamNumber>,
	pub blue_alliance: Vec<TeamNumber>,
}

/// Number for a match
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, Hash, Default)]
pub struct MatchNumber {
	pub ty: MatchType,
	pub num: u16,
}

impl Display for MatchNumber {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}{}",
			match self.ty {
				MatchType::Qualification => "Q",
				MatchType::Playoff => "P",
				MatchType::Finals => "F",
			},
			self.num
		)
	}
}

impl FromStr for MatchNumber {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		if s.len() < 2 {
			return Err(());
		}

		let ty = &s[0..1];
		let ty = match ty {
			"Q" => MatchType::Qualification,
			"P" | "S" => MatchType::Playoff,
			"F" => MatchType::Finals,
			_ => return Err(()),
		};

		let Ok(num) = (&s[1..]).parse::<u16>() else {
			return Err(());
		};

		Ok(Self { ty, num })
	}
}

/// Type of a match
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MatchType {
	#[default]
	Qualification,
	Playoff,
	Finals,
}

impl Display for MatchType {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::Qualification => "Qualification",
				Self::Playoff => "Playoff",
				Self::Finals => "Finals",
			}
		)
	}
}

impl FromStr for MatchType {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"Qualification" | "Q" => Ok(Self::Qualification),
			"Playoff" | "P" | "S" => Ok(Self::Playoff),
			"Finals" | "F" => Ok(Self::Finals),
			_ => Err(()),
		}
	}
}

/// Unique identifier for match stats made from information like the match number, competition, and recorder
#[derive(Clone, PartialEq, Eq)]
pub struct MatchStatsID(String);

impl MatchStatsID {
	pub fn new(
		team_number: TeamNumber,
		match_number: Option<MatchNumber>,
		competition: Option<Competition>,
		recorder: Option<&str>,
	) -> Self {
		Self(format!(
			"{team_number}.{}.{}.{}",
			match_number.unwrap_or_default(),
			competition.map(|x| x.to_string()).unwrap_or_default(),
			recorder.unwrap_or_default(),
		))
	}

	pub fn from_str(string: String) -> Self {
		Self(string)
	}
}

impl Display for MatchStatsID {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

/// Stats for a single team in a match
#[derive(Serialize, Deserialize, Clone)]
pub struct MatchStats {
	/// The team that got these stats
	pub team_number: TeamNumber,
	/// The match where these stats occurred
	pub match_id: String,
	/// The match number for these stats
	#[serde(default)]
	pub match_number: Option<MatchNumber>,
	/// The member who recorded these stats
	#[serde(default)]
	pub recorder: Option<String>,
	/// When the stats were recorded, as a DateTime
	#[serde(default)]
	pub record_time: Option<String>,
	/// Whether this match happened live when it was recorded
	#[serde(default)]
	pub recorded_live: bool,
	/// The competition that this match is associated with
	#[serde(default)]
	pub competition: Option<Competition>,
	/// The auto that the team ran during this match
	#[serde(default)]
	pub auto: Option<String>,
	/// The auto fuel attempts
	pub auto_fuel_volleys: Vec<EventVolley>,
	/// The auto intake attempts
	pub auto_intake_volleys: Vec<EventVolley>,
	/// Whether or not an auto climb was attempted
	pub auto_climb_attempted: bool,
	/// Whether or not an auto climb was successful
	pub auto_climb_successful: bool,
	/// Whether or not the robot collided with another during auto
	pub auto_collision: bool,
	/// The fuel attempts during teleop
	pub teleop_fuel_volleys: Vec<EventVolley>,
	/// The fuel intake attempts during teleop
	pub teleop_intake_volleys: Vec<EventVolley>,
	/// The pass attempts during teleop
	pub teleop_pass_volleys: Vec<EventVolley>,
	/// The climb that the team attempted to do
	pub climb_attempted: ClimbAbility,
	/// The result of the climb
	pub climb_result: ClimbResult,
	/// How long the climb took
	pub climb_time: f32,
	/// The total number of points that the team scored
	pub points_scored: i16,
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
	/// The broken status of the robot
	#[serde(default)]
	pub status: RobotStatus,
	/// Whether or not the team showed up to the match
	pub showed_up: bool,
	/// Whether or not the team won the match
	#[serde(default)]
	pub won: bool,
	/// Additional notes about the match
	pub notes: String,
	/// Team strengths during the match
	#[serde(default)]
	pub strengths: String,
	/// Team weaknesses during the match
	#[serde(default)]
	pub weaknesses: String,
	/// Whether the team left in auto
	#[serde(default)]
	pub auto_leave: bool,
	/// Whether the robot had brownout issues
	#[serde(default)]
	pub brownout: bool,
	/// Whether the robot had tipping issues
	#[serde(default)]
	pub tipping: bool,
	/// Whether the robot was beached on an algae
	#[serde(default)]
	pub beached: bool,
	/// Whether the robot had a large pause when teleop started
	#[serde(default)]
	pub teleop_pause: bool,
	/// Whether the robot had a stuck game piece
	#[serde(default)]
	pub game_piece_stuck: bool,
	/// Data from the match report
	#[serde(default)]
	pub match_report_data: Option<serde_json::Map<String, serde_json::Value>>,
}

impl MatchStats {
	pub fn get_id(&self) -> MatchStatsID {
		MatchStatsID::new(
			self.team_number,
			self.match_number.clone(),
			self.competition,
			self.recorder.as_deref(),
		)
	}
}

/// A series of fuel shots or pickups
#[derive(Serialize, Deserialize, Clone)]
pub struct EventVolley {
	/// How many shots were attempted
	pub shots_attempted: u8,
	/// How many shots were made
	pub shots_made: u8,
	/// Game time where the event started
	pub start_time: f32,
	/// Game time where the event ended
	pub end_time: f32,
}

impl EventVolley {
	/// Gets the duration of this volley if the timing data is available
	pub fn duration(&self) -> Option<f32> {
		if self.start_time == 0.0 || self.end_time == 0.0 || self.start_time >= self.end_time {
			None
		} else {
			Some(self.end_time - self.start_time)
		}
	}

	/// Gets the speed / fire rate of this volley, if the timing data is available
	pub fn get_rate(&self) -> Option<f32> {
		if let Some(duration) = self.duration() {
			Some(self.shots_attempted as f32 / duration)
		} else {
			None
		}
	}
}

/// Count how many matches a member has scouted
pub fn count_matches_scouted(
	member: &str,
	matches: &[MatchStats],
	current_competition: Option<&Competition>,
) -> usize {
	matches
		.iter()
		.filter(|x| {
			x.recorder.as_ref().is_some_and(|x| x == member)
				&& x.competition.is_some()
				&& x.competition.as_ref() == current_competition
		})
		.count()
}
