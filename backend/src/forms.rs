use std::fmt::Display;

use serde::{Deserialize, Serialize};

/// Different types of forms
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum Form {
	ConsentRelease,
	TeamFees,
	ToolDrillPress,
	ToolPowerDrill,
	ToolMetalPress,
	ToolTableSaw,
	ToolHorizontalBandsaw,
	ToolPowerSander,
	ToolMiterSaw,
	ToolWoodBandsaw,
	ToolMetalBandsaw,
	ToolHandTools,
}

impl Form {
	/// Gets if this form is necessary or optional
	pub fn is_optional(&self) -> bool {
		match self {
			Self::ConsentRelease | Self::TeamFees => false,
			_ => true,
		}
	}
}

impl Display for Form {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::ConsentRelease => "Consent-Release Form",
				Self::TeamFees => "Team Fees",
				Form::ToolDrillPress => "Drill Press Certification",
				Form::ToolPowerDrill => "Power Drill Certification",
				Form::ToolMetalPress => "Metal Press Certification",
				Form::ToolTableSaw => "Table Saw Certification",
				Form::ToolHorizontalBandsaw => "Horizontal Bandsaw Certification",
				Form::ToolPowerSander => "Power Sander Certification",
				Form::ToolMiterSaw => "Miter Saw Certification",
				Form::ToolWoodBandsaw => "Wood Bandsaw Certification",
				Form::ToolMetalBandsaw => "Metal Bandsaw Certification",
				Form::ToolHandTools => "Hand Tools Certification",
			}
		)
	}
}
