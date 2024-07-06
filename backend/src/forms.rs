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
