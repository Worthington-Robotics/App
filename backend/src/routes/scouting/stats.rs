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
		title.replace(STAT_FUEL, "Fuel")
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

/// Icon for fuel in stat cards
pub static STAT_FUEL: &str =
	"<img src=\"/assets/icons/fuel.svg\" style=\"width:1.2rem;margin-right:-0.5rem\" />";

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
	ranking_points, "ranking_points" => "RP", "Ranking Points", "Average number of ranking points contributed";
	fuel_rp, "fuel_rp" => "FRP", "Fuel RP", "Average number of ranking points contributed for fuel";
	climb_rp, "climb_rp" => "CRP", "Climb RP", "Average number of ranking points contributed for climbing";
	teleop_score, "teleop_score" => "TSCO", "Teleop Score", "Average number of points scored in teleop";
	active_efficiency, "active_efficiency" => "AE", "Active Efficiency", "% of time spent scoring during active shifts";
	inactive_efficiency, "inactive_efficiency" => "IE", "Inactive Efficiency", "% of time spent intaking during inactive shifts";
	fuel_score, "fuel_score" => "FSCO", "Fuel Score", "Average number of points scored from fuel";
	fuel_accuracy, "fuel_accuracy" => "FACC", "Fuel Accuracy", "% of fuel shots that go in";
	fuel_speed, "fuel_speed" => "FSPD", "Fuel Speed", "Fuel shots per second";
	fuel_per_volley, "fuel_per_volley" => "FPV", "Fuel Per Volley", "Average fuel per group of shots";
	intake_speed, "intake_speed" => "ISPD", "Intake Speed", "Fuel intakes per second";
	fuel_per_intake, "fuel_per_intake" => "FPI", "Fuel Per Intake", "Average fuel per group of intakes";
	pass_average, "pass_average" => "PAVG", "Pass Average", "Average number of fuel passes per match";
	fuel_per_pass, "fuel_per_pass" => "FPP", "Fuel Per Pass", "Average number of fuel in each group of passes";
	climb_accuracy, "climb_accuracy" => "CACC", "Climb Accuracy", "Climb success rate";
	climb_time, "climb_time" => "CLT", "Climb Time", "Average time to climb";
	climb_fall_percent, "climb_fall_percent" => "CFP", "Climb Fall Percent", "Rate at which the team falls when climbing";
	climb_score, "climb_score" => "CSCO", "Climb Score", "Average number of points scored from climbing";
	auto_fuel, "auto_fuel" => "AF", "Auto Fuel", "Average number of fuel scored during auto";
	auto_fuel_accuracy, "auto_fuel_accuracy" => "AFACC", "Auto Fuel Accuracy", "% of shots made during auto";
	auto_climb_accuracy, "auto_climb_accuracy" => "ACACC", "Auto Climb Accuracy", "% of success climbing in auto";
	auto_collisions, "auto_collisions" => "ACOL", "Auto Collisions", "Total times this team hit another robot during auto";
	auto_score, "auto_score" => "ASCO", "Auto Score", "Average number of points scored in auto";
	cycle_time, "cycle_time" => "CT", "Cycle Time", "Average time between intake, score, and the next intake";
	cycle_time_consistency, "cycle_time_consistency" => "CTC", "Cycle Time Consistency", "How close to a linear fit this team's cycles are";
	cycle_time_deviation, "cycle_time_deviation" => "CTD", "Cycle Time Deviation", "Standard deviation of this team's cycle times";
	penalties, "penalties" => "Pen", "Penalties", "Total number of penalties across all matches";
	reliability, "reliability" => "RB", "Reliability", "How often this team plays a match without breaking";
	matches, "matches" => "MA", "Matches", "How many matches have been scouted for this team";
	total_points, "total_points" => "TP", "Total Points", "How many points this team has scored in all matches";
	total_fuel, "total_fuel" => "TF", "Total Fuel", "How many fuel this team has scored in all matches";
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
