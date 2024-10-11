// Macros for rendering stat cards that include breakdowns

macro_rules! stat_card {
	($f: path, $team_stats:expr, $title: literal, $stat: ident, $stat_id: literal, $important: literal) => {
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
	($team_stats: expr, $title: literal, $stat: ident, $stat_id: literal, $important: literal) => {
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
	($team_stats: expr, $title: literal, $stat: ident, $stat_id: literal, $important: literal) => {
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
	($team_stats: expr, $title: literal, $stat: ident, $stat_id: literal, $important: literal) => {
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

use crate::util::fix_empty_string;
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
	let out = out.replace("{{data-title}}", &format!("\"{title}\""));

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
