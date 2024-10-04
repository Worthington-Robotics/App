use std::{fmt::Display, str::FromStr};

use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

/// Different types of forms
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, EnumIter)]
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

	pub fn to_db(&self) -> &'static str {
		match self {
			Self::ConsentRelease => "ConsentRelease",
			Self::TeamFees => "TeamFees",
			Form::ToolDrillPress => "DrillPress",
			Form::ToolPowerDrill => "PowerDrill",
			Form::ToolMetalPress => "MetalPress",
			Form::ToolTableSaw => "TableSaw",
			Form::ToolHorizontalBandsaw => "HorizontalBandsaw",
			Form::ToolPowerSander => "PowerSander",
			Form::ToolMiterSaw => "MiterSaw",
			Form::ToolWoodBandsaw => "WoodBandsaw",
			Form::ToolMetalBandsaw => "MetalBandsaw",
			Form::ToolHandTools => "HandTools",
		}
	}
}

impl FromStr for Form {
	type Err = ();

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"ConsentRelease" => Ok(Self::ConsentRelease),
			"TeamFees" => Ok(Self::TeamFees),
			"DrillPress" => Ok(Self::ToolDrillPress),
			"PowerDrill" => Ok(Self::ToolPowerDrill),
			"MetalPress" => Ok(Self::ToolMetalPress),
			"TableSaw" => Ok(Self::ToolTableSaw),
			"HorizontalBandsaw" => Ok(Self::ToolHorizontalBandsaw),
			"PowerSander" => Ok(Self::ToolPowerSander),
			"MiterSaw" => Ok(Self::ToolMiterSaw),
			"WoodBandsaw" => Ok(Self::ToolWoodBandsaw),
			"MetalBandsaw" => Ok(Self::ToolMetalBandsaw),
			"HandTools" => Ok(Self::ToolHandTools),
			_ => Err(()),
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
