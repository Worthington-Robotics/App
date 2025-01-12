use serde::{Deserialize, Serialize};

/// Different climbing capabilities
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClimbAbility {
	None,
	Shallow,
	Deep,
}

/// Level for the reef
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ReefLevel {
	L1,
	L2,
	L3,
	L4,
}

/// Game pieces
#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GamePiece {
	Coral,
	Algae,
}

/// Gets the point value of a coral
pub fn get_coral_points(level: ReefLevel, is_auto: bool) -> u8 {
	if is_auto {
		match level {
			ReefLevel::L1 => 3,
			ReefLevel::L2 => 4,
			ReefLevel::L3 => 6,
			ReefLevel::L4 => 7,
		}
	} else {
		match level {
			ReefLevel::L1 => 2,
			ReefLevel::L2 => 3,
			ReefLevel::L3 => 4,
			ReefLevel::L4 => 5,
		}
	}
}
