use serde::{Deserialize, Serialize};

use super::{matches::MatchNumber, TeamNumber};

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct ScoutingAssignment {
	pub member: String,
	pub teams: Vec<TeamNumber>,
}

/// Randomly assigns teams to scouts
pub fn assign_scouts(teams: &[TeamNumber], members: &[String]) -> Vec<ScoutingAssignment> {
	if members.is_empty() {
		return Vec::new();
	}

	let mut out: Vec<_> = members
		.iter()
		.filter(|x| *x != "admin")
		.map(|x| ScoutingAssignment {
			member: x.clone(),
			..Default::default()
		})
		.collect();

	let mut current_member = 0;
	for team in teams {
		out[current_member].teams.push(*team);

		current_member += 1;
		if current_member >= out.len() {
			current_member = 0;
		}
	}

	out.push(ScoutingAssignment {
		member: "admin".into(),
		..Default::default()
	});

	out
}

/// Scout claims for a single match
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct MatchClaims {
	/// The match number
	pub m: MatchNumber,
	pub red_1: Option<String>,
	pub red_2: Option<String>,
	pub red_3: Option<String>,
	pub blue_1: Option<String>,
	pub blue_2: Option<String>,
	pub blue_3: Option<String>,
}
