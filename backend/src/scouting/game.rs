use serde::{Deserialize, Serialize};

/// Different climbing capabilities
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClimbAbility {
	None,
	L1,
	L2,
	L3,
}

impl ClimbAbility {
	/// Gets the score of this climb in endgame (not auto)
	pub fn get_score(&self) -> u8 {
		match self {
			Self::None => 0,
			Self::L1 => 10,
			Self::L2 => 20,
			Self::L3 => 30,
		}
	}
}

/// Result stating the success of a climb
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClimbResult {
	Failed,
	Fell,
	Succeeded,
}
