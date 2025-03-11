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
	let long_title = if let Some(result) = get_team_stat_display_name(id) {
		result.1.to_string()
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

/// Gets the display name of a team stat, returning both the short and long version
pub fn get_team_stat_display_name(stat: &str) -> Option<(&'static str, &'static str)> {
	match stat {
		"apa" => Some(("APA", "Actual Points Added")),
		"win_rate" => Some(("WR", "Win Rate")),
		"coral_score" => Some(("CSCO", "Coral Score")),
		"coral_average" => Some(("CAVG", "Coral Average")),
		"coral_accuracy" => Some(("CACC", "Coral Accuracy")),
		"algae_score" => Some(("ASCO", "Algae Score")),
		"processor_average" => Some(("PAVG", "Processor Average")),
		"processor_accuracy" => Some(("PACC", "Processor Accuracy")),
		"net_average" => Some(("NAVG", "Net Average")),
		"intake_accuracy" => Some(("IACC", "Intake Accuracy")),
		"climb_accuracy" => Some(("CACC", "Climb Accuracy")),
		"climb_time" => Some(("CLT", "Climb Time")),
		"climb_fall_percent" => Some(("CFP", "Climb Fall Percent")),
		"auto_coral" => Some(("AC", "Auto Coral")),
		"auto_algae" => Some(("AA", "Auto Algae")),
		"auto_coral_accuracy" => Some(("ACA", "Auto Coral Accuracy")),
		"auto_algae_accuracy" => Some(("AAA", "Auto Algae Accuracy")),
		"auto_collisions" => Some(("ACOL", "Auto Collisions")),
		"offense_average" => Some(("OA", "Offense Average")),
		"defense_average" => Some(("DA", "Defense Average")),
		"cycle_time" => Some(("CT", "Cycle Time")),
		"cycle_time_consistency" => Some(("CTC", "Cycle Time Consistency")),
		"cycle_time_deviation" => Some(("CTD", "Cycle Time Deviation")),
		"time_to_first_cycle" => Some(("TTFC", "Time To First Cycle")),
		"penalties" => Some(("Pen", "Penalties")),
		"reliability" => Some(("RB", "Reliability")),
		"matches" => Some(("Matches", "Matches")),
		"auto_score" => Some(("ATSCO", "Auto Score")),
		"teleop_score" => Some(("TESCO", "Teleop Score")),
		"climb_score" => Some(("CLSCO", "Climb Score")),
		"l1_accuracy" => Some(("L1ACC", "L1 Accuracy")),
		"l2_accuracy" => Some(("L2ACC", "L2 Accuracy")),
		"l3_accuracy" => Some(("L3ACC", "L3 Accuracy")),
		"l4_accuracy" => Some(("L4ACC", "L4 Accuracy")),
		"l1_value" => Some(("L1VAL", "L1 Value")),
		"l2_value" => Some(("L2VAL", "L2 Value")),
		"l3_value" => Some(("L3VAL", "L3 Value")),
		"l4_value" => Some(("L4VAL", "L4 Value")),
		"l1_count" => Some(("L1CNT", "L1 Count")),
		"l2_count" => Some(("L2CNT", "L2 Count")),
		"l3_count" => Some(("L3CNT", "L3 Count")),
		"l4_count" => Some(("L4CNT", "L4 Count")),
		_ => None,
	}
}
