use std::{fmt::Display, str::FromStr};

use rocket::FromFormField;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use crate::util::ToDropdown;

#[derive(Serialize, Deserialize, Clone)]
pub struct Checklist {
	pub id: String,
	pub name: String,
	/// List of task IDs
	pub tasks: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Task {
	pub id: String,
	pub checklist: String,
	pub text: String,
	pub done: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, EnumIter, FromFormField)]
pub enum ChecklistTemplate {
	TeamsAtCompetition,
}

impl Display for ChecklistTemplate {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::TeamsAtCompetition => "Teams at This Competition",
			}
		)
	}
}

impl FromStr for ChecklistTemplate {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"teams_at_competition" => Ok(Self::TeamsAtCompetition),
			_ => Err(()),
		}
	}
}

impl ToDropdown for ChecklistTemplate {
	fn to_dropdown(&self) -> &'static str {
		match self {
			Self::TeamsAtCompetition => "TeamsAtCompetition",
		}
	}
}
