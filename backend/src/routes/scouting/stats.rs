// Macros for rendering stat cards that include breakdowns

macro_rules! stat_card {
	($f: path, $team_stats:expr, $title: expr, $stat: ident, $stat_id: literal, $important: literal) => {
		&{
			let all_time = $f(
				$title,
				$stat_id,
				$team_stats.all_time.$stat,
				$important,
				"non-comp",
			);
			let current_competition = $f(
				$title,
				$stat_id,
				$team_stats.current_competition.$stat,
				$important,
				"comp",
			);

			format!("{all_time}{current_competition}")
		}
	};
}

macro_rules! stat_card_float {
	($team_stats: expr, $title: expr, $stat: ident, $stat_id: literal, $important: literal) => {
		crate::routes::scouting::stats::stat_card!(
			crate::routes::scouting::stats::render_stat_card_float,
			$team_stats,
			$title,
			$stat,
			$stat_id,
			$important
		)
	};
}

macro_rules! stat_card_pct {
	($team_stats: expr, $title: expr, $stat: ident, $stat_id: literal, $important: literal) => {
		crate::routes::scouting::stats::stat_card!(
			crate::routes::scouting::stats::render_stat_card_pct,
			$team_stats,
			$title,
			$stat,
			$stat_id,
			$important
		)
	};
}

macro_rules! stat_card_other {
	($team_stats: expr, $title: expr, $stat: ident, $stat_id: literal, $important: literal) => {
		crate::routes::scouting::stats::stat_card!(
			crate::routes::scouting::stats::render_stat_card,
			$team_stats,
			$title,
			$stat,
			$stat_id,
			$important
		)
	};
}

use itertools::Itertools;

use crate::scouting::stats::TeamStats;
use crate::util::{escape_html, fix_empty_string};
pub(crate) use {stat_card, stat_card_float, stat_card_other, stat_card_pct};

// Functions for rendering stat cards

pub fn render_stat_card(
	title: &str,
	id: &str,
	stat: impl std::fmt::Display,
	strong: bool,
	class: &str,
) -> String {
	let out = include_str!("../components/scouting/stat_card.min.html");
	let out = out.replace("{{stat}}", &stat.to_string());
	let out = out.replace("{{id}}", fix_empty_string(id));

	let out = out.replace("{{title}}", title);
	let long_title = if let Some(result) = StatInfo::get(id) {
		result.name.to_string()
	} else {
		title
			.replace(STAT_CORAL, "Coral")
			.replace(STAT_ALGAE, "Algae")
	};
	let fixed_title = format!("\"{}\"", escape_html(&long_title));
	let out = out.replace("{{data-title}}", &fixed_title);

	let stat_class = if strong { "strong" } else { "" };
	let out = out.replace("{{stat-class}}", stat_class);

	let out = out.replace("{{card-class}}", class);

	out
}

pub fn render_stat_card_float(
	title: &str,
	id: &str,
	stat: f32,
	strong: bool,
	class: &str,
) -> String {
	render_stat_card(title, id, format!("{stat:.2}"), strong, class)
}

pub fn render_stat_card_pct(title: &str, id: &str, stat: f32, strong: bool, class: &str) -> String {
	render_stat_card(title, id, format!("{:.1}%", stat * 100.0), strong, class)
}

pub fn render_stat_card_optional(
	title: &str,
	id: &str,
	stat: Option<impl std::fmt::Display>,
	strong: bool,
	class: &str,
) -> String {
	if let Some(stat) = stat {
		render_stat_card(title, id, stat, strong, class)
	} else {
		render_stat_card(title, id, "?", strong, class)
	}
}

pub fn render_stat_card_optional_bool(
	title: &str,
	id: &str,
	stat: Option<bool>,
	strong: bool,
	class: &str,
) -> String {
	if let Some(stat) = stat {
		render_stat_card(title, id, if stat { "Yes" } else { "No" }, strong, class)
	} else {
		render_stat_card(title, id, "?", strong, class)
	}
}

pub fn render_stat_card_optional_float(
	title: &str,
	id: &str,
	stat: Option<f32>,
	strong: bool,
	class: &str,
) -> String {
	if let Some(stat) = stat {
		render_stat_card_float(title, id, stat, strong, class)
	} else {
		render_stat_card(title, id, "?", strong, class)
	}
}

/// Icon for coral in stat cards
pub static STAT_CORAL: &str =
	"<img src=\"/assets/icons/coral.svg\" style=\"width:0.75rem;margin-right:-0.5rem\" />";
/// Icon for algae in stat cards
pub static STAT_ALGAE: &str =
	"<img src=\"/assets/icons/algae.svg\" style=\"width:1.2rem;margin-right:-0.5rem\" />";

/// Info about a team stat
pub struct StatInfo {
	/// The full name of the stat
	pub name: &'static str,
	/// The short abbreviation for the stat
	pub abbreviation: &'static str,
	/// A short description of the stat
	pub description: &'static str,
}

macro_rules! stat_info {
	($($ident:ident, $id:literal => $abbr:literal, $name:literal, $description:literal);+$(;)?) => {
		pub static ALL_STATS: &'static [&'static str] = &[
			$(
				$id,
			)+
		];

		impl StatInfo {
			pub fn get(stat: &str) -> Option<StatInfo> {
				match stat {
					$(
						$id => Some(StatInfo {
							name: $name,
							abbreviation: $abbr,
							description: $description,
						}),
					)+
					_ => None,
				}
			}

			pub fn get_stat_value(stats: &TeamStats, stat: &str) -> Option<f32> {
				match stat {
					$(
						$id => Some(stats.$ident as f32),
					)+
					_ => None,
				}
			}
		}
	};
}

stat_info! {
	apa, "apa" => "APA", "Average Points Added", "The average number of points that this team scores";
	win_rate, "win_rate" => "WR", "Win Rate", "How often this team wins";
	coral_score, "coral_score" => "CSCO", "Coral Score", "Average points from coral in teleop";
	coral_average, "coral_average" => "CAVG", "Coral Average", "Average number of coral scored in teleop";
	coral_accuracy, "coral_accuracy" => "CACC", "Coral Accuracy", "Success rate for scoring coral";
	algae_score, "algae_score" => "ALSCO", "Algae Score", "Average points from algae in teleop";
	processor_average, "processor_average" => "PAVG", "Processor Average", "Average number of teleop algae scored in the processor";
	processor_accuracy, "processor_accuracy" => "PACC", "Processor Accuracy", "Success rate for the processor";
	net_average, "net_average" => "NAVG", "Net Average", "Average number of net scores";
	intake_accuracy, "intake_accuracy" => "IACC", "Intake Accuracy", "Intake success rate in teleop";
	climb_accuracy, "climb_accuracy" => "CACC", "Climb Accuracy", "Climb success rate";
	climb_time, "climb_time" => "CLT", "Climb Time", "Average time to climb";
	climb_fall_percent, "climb_fall_percent" => "CFP", "Climb Fall Percent", "Rate at which the team falls when climbing";
	auto_coral, "auto_coral" => "AC", "Auto Coral", "Average number of coral scored in auto";
	auto_algae, "auto_algae" => "AA", "Auto Algae", "Average number of algae scored in auto";
	auto_coral_accuracy, "auto_coral_accuracy" => "ACA", "Auto Coral Accuracy", "Success rate for scoring coral in auto";
	auto_algae_accuracy, "auto_algae_accuracy" => "AAA", "Auto Algae Accuracy", "Success rate for scoring algae in auto";
	auto_intake_accuracy, "auto_intake_accuracy" => "AINTK", "Auto Intake Accuracy", "Success rate for intaking in auto";
	auto_collisions, "auto_collisions" => "ACOL", "Auto Collisions", "Total times this team hit another robot during auto";
	offense_average, "offense_average" => "OA", "Offense Average", "Average number of offensive moves";
	defense_average, "defense_average" => "DA", "Defense Average", "Average number of defensive moves";
	cycle_time, "cycle_time" => "CT", "Cycle Time", "Average time between intake, score, and the next intake";
	cycle_time_consistency, "cycle_time_consistency" => "CTC", "Cycle Time Consistency", "How close to a linear fit this team's cycles are";
	cycle_time_deviation, "cycle_time_deviation" => "CTD", "Cycle Time Deviation", "Standard deviation of this team's cycle times";
	time_to_first_cycle, "time_to_first_cycle" => "TTFC", "Time To First Cycle", "Average time before the first score in teleop";
	penalties, "penalties" => "Pen", "Penalties", "Total number of penalties across all matches";
	reliability, "reliability" => "RB", "Reliability", "How often this team plays a match without breaking";
	matches, "matches" => "MA", "Matches", "How many matches have been scouted for this team";
	auto_score, "auto_score" => "ASCO", "Auto Score", "Average number of points scored in auto";
	teleop_score, "teleop_score" => "TSCO", "Teleop Score", "Average number of points scored in teleop";
	climb_score, "climb_score" => "CSCO", "Climb Score", "Average number of points scored from climbing";
	l1_accuracy, "l1_accuracy" => "L1ACC", "L1 Accuracy", "Success rate scoring on L1";
	l2_accuracy, "l2_accuracy" => "L2ACC", "L2 Accuracy", "Success rate scoring on L2";
	l3_accuracy, "l3_accuracy" => "L3ACC", "L3 Accuracy", "Success rate scoring on L3";
	l4_accuracy, "l4_accuracy" => "L4ACC", "L4 Accuracy", "Success rate scoring on L4";
	l1_value, "l1_value" => "L1VAL", "L1 Value", "Average points gained from a single L1 score attempt";
	l2_value, "l2_value" => "L2VAL", "L2 Value", "Average points gained from a single L2 score attempt";
	l3_value, "l3_value" => "L3VAL", "L3 Value", "Average points gained from a single L3 score attempt";
	l4_value, "l4_value" => "L4VAL", "L4 Value", "Average points gained from a single L4 score attempt";
	l1_count, "l1_count" => "L1CNT", "L1 Count", "Total times this team has scored on L1";
	l2_count, "l2_count" => "L2CNT", "L2 Count", "Total times this team has scored on L2";
	l3_count, "l3_count" => "L3CNT", "L3 Count", "Total times this team has scored on L3";
	l4_count, "l4_count" => "L4CNT", "L4 Count", "Total times this team has scored on L4";
	litter, "litter" => "LTTR", "Litter", "The amount of pieces dropped, with algae being worth 3 coral";
	coral_rp_contribution, "coral_rp_contribution" => "CRP", "Coral RP Contribution", "How much of the coral RP this team contributes";
	barge_rp_contribution, "barge_rp_contribution" => "BRP", "Barge RP Contribution", "How much of the barge RP this team contributes";
	total_points, "total_points" => "TP", "Total Points", "How many points this team has scored in all matches";
	total_coral, "total_coral" => "TC", "Total Coral", "How many coral this team has scored in all matches";
	total_algae, "total_algae" => "TA", "Total Algae", "How many algae this team has scored in all matches";
	high_score, "high_score" => "HS", "High Score", "The highest number of points this team has scored";
}

/// Creates a dropdown of team stat options
pub fn create_stat_dropdown_options() -> String {
	let mut out = String::new();
	for stat in std::iter::once(&"none").chain(ALL_STATS.into_iter().sorted()) {
		let info = StatInfo::get(stat).unwrap_or(StatInfo {
			name: "None",
			abbreviation: "",
			description: "",
		});
		let selected_str = if *stat == "none" { " selected" } else { "" };
		let option = format!(
			"<option value=\"{}\"{selected_str}>{}</option>",
			stat, info.name,
		);
		out.push_str(&option);
	}

	out
}
