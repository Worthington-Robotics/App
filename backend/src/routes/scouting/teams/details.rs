use rocket::{
	http::Status,
	response::{content::RawHtml, Redirect},
};
use strum::IntoEnumIterator;
use tracing::{error, span, Level};

use crate::{
	db::Database,
	routes::OptionalSessionID,
	scouting::{
		game::GamePiece, stats::CombinedTeamStats, status::RobotStatus, Competition,
		DriveTrainType, TeamNumber,
	},
	State,
};

use crate::routes::scouting::{
	create_page,
	stats::{
		render_stat_card, render_stat_card_float, render_stat_card_optional,
		render_stat_card_optional_bool, render_stat_card_optional_float, stat_card_float,
		stat_card_other, stat_card_pct, STAT_ALGAE, STAT_CORAL,
	},
	PageOrRedirect, Scope,
};

#[rocket::get("/scouting/team/<id>?<competition>")]
pub async fn team_details(
	id: TeamNumber,
	session_id: OptionalSessionID<'_>,
	state: &State,
	competition: Option<&str>,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Team details page");
	let _enter = span.enter();

	let competition_str = competition.unwrap_or("Current");

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	let Ok(requesting_member) = session_id.get_requesting_member(state).await else {
		return Ok(redirect);
	};

	let lock = state.db.read().await;
	let team = lock
		.get_team(id)
		.await
		.map_err(|e| {
			error!("Failed to get team from database: {e}");
			Status::InternalServerError
		})?
		.ok_or_else(|| {
			error!("Team does not exist: {}", id);
			Status::NotFound
		})?;

	let page = include_str!("../../pages/scouting/team/details.min.html");
	let page = page.replace("{{name}}", &team.name);
	let page = page.replace("{{number}}", &team.number.to_string());
	let page = page.replace("__team_number__", &team.number.to_string());
	let page = page.replace("{{rookie-year}}", &team.rookie_year.to_string());
	let page = page.replace("{{competition}}", competition_str);

	// Follow button
	let is_following = team.followers.contains(&requesting_member.id);
	let star_display = if is_following { "" } else { "none" };
	let star_outline_display = if is_following { "none" } else { "" };
	let page = page.replace("{{star-display}}", star_display);
	let page = page.replace("{{outline-display}}", star_outline_display);

	let status_updates = lock.get_team_status(team.number).await.map_err(|e| {
		error!("Failed to get team status updates from database: {e}");
		Status::InternalServerError
	})?;

	// Status
	let current_status = RobotStatus::get_from_updates(status_updates.iter());
	let page = page.replace("{{status}}", &current_status.to_string());
	let page = page.replace("{{status-color}}", current_status.get_color());

	// Create checkboxes for changing competition status
	let disabled_attr = if requesting_member.is_elevated() {
		""
	} else {
		" disabled"
	};
	let mut checkboxes_string = String::new();
	for comp in Competition::iter() {
		let checked_attr = if team.competitions.contains(&comp) {
			" checked"
		} else {
			""
		};

		let component = format!(
			r#"<div class="cont round comp-cb"><input type=checkbox {disabled_attr} {checked_attr} data-val={comp} /> {}</div>"#,
			comp.get_abbr()
		);
		checkboxes_string.push_str(&component);
	}
	let page = page.replace("{{comp-checkboxes}}", &checkboxes_string);

	let page = page.replace(
		"{{edit-button}}",
		include_str!("../../components/ui/edit.min.html"),
	);

	// Create stats
	let epa = state
		.statbotics_client
		.get_epa(id)
		.await
		.unwrap_or_default();
	let page = page.replace("{{epa}}", &render_stat_card_float("EPA", "", epa, true, ""));

	let default_stats = CombinedTeamStats::default();
	let lock2 = state.team_stats.read().await;
	let team_stats = lock2.get(&id).unwrap_or(&default_stats);
	let page = page.replace(
		"{{apa}}",
		stat_card_float!(team_stats, "APA", apa, "apa", true),
	);
	let page = page.replace(
		"{{win-rate}}",
		stat_card_pct!(team_stats, "Win Rate", win_rate, "win_rate", true),
	);
	let page = page.replace(
		"{{matches}}",
		stat_card_other!(team_stats, "Matches", matches, "matches", false),
	);
	let page = page.replace(
		"{{reliability}}",
		stat_card_pct!(team_stats, "Reliability", reliability, "reliability", false),
	);
	let page = page.replace(
		"{{penalties}}",
		stat_card_other!(team_stats, "Penalties", penalties, "penalties", false),
	);
	let page = page.replace(
		"{{coral-rp-contribution}}",
		stat_card_pct!(
			team_stats,
			"Coral RP",
			coral_rp_contribution,
			"coral_rp_contribution",
			false
		),
	);
	let page = page.replace(
		"{{barge-rp-contribution}}",
		stat_card_pct!(
			team_stats,
			"Barge RP",
			barge_rp_contribution,
			"barge_rp_contribution",
			false
		),
	);
	let page = page.replace(
		"{{litter}}",
		stat_card_float!(team_stats, "Litter", litter, "litter", false),
	);
	let page = page.replace(
		"{{auto-score}}",
		stat_card_float!(team_stats, "Score", auto_score, "auto_score", true),
	);
	let page = page.replace(
		"{{auto-coral}}",
		stat_card_float!(team_stats, STAT_CORAL, auto_coral, "auto_coral", true),
	);
	let page = page.replace(
		"{{auto-algae}}",
		stat_card_float!(team_stats, STAT_ALGAE, auto_algae, "auto_algae", true),
	);
	let page = page.replace(
		"{{auto-coral-accuracy}}",
		stat_card_pct!(
			team_stats,
			&format!("{STAT_CORAL} Acc"),
			auto_coral_accuracy,
			"auto_coral_accuracy",
			true
		),
	);
	let page = page.replace(
		"{{auto-algae-accuracy}}",
		stat_card_pct!(
			team_stats,
			&format!("{STAT_ALGAE} Avg"),
			auto_algae_accuracy,
			"auto_algae_accuracy",
			true
		),
	);
	let page = page.replace(
		"{{auto-intake-accuracy}}",
		stat_card_pct!(
			team_stats,
			&format!("Intake Acc"),
			auto_intake_accuracy,
			"auto_intake_accuracy",
			false
		),
	);
	let page = page.replace(
		"{{auto-collisions}}",
		stat_card_other!(
			team_stats,
			"Collisions",
			auto_collisions,
			"auto_collisions",
			false
		),
	);
	let page = page.replace(
		"{{cycle-time}}",
		stat_card_float!(team_stats, "CT", cycle_time, "cycle_time", true),
	);
	let page = page.replace(
		"{{cycle-time-consistency}}",
		stat_card_pct!(
			team_stats,
			"CTC",
			cycle_time_consistency,
			"cycle_time_consistency",
			true
		),
	);
	let page = page.replace(
		"{{cycle-time-deviation}}",
		stat_card_float!(
			team_stats,
			"CTD",
			cycle_time_deviation,
			"cycle_time_deviation",
			true
		),
	);
	let page = page.replace(
		"{{teleop-score}}",
		stat_card_float!(team_stats, "Score", teleop_score, "teleop_score", true),
	);
	let page = page.replace(
		"{{coral-score}}",
		stat_card_float!(
			team_stats,
			&format!("{STAT_CORAL} Sco"),
			coral_score,
			"coral_score",
			true
		),
	);
	let page = page.replace(
		"{{coral-average}}",
		stat_card_float!(
			team_stats,
			&format!("{STAT_CORAL} Avg"),
			coral_average,
			"coral_average",
			true
		),
	);
	let page = page.replace(
		"{{coral-accuracy}}",
		stat_card_pct!(
			team_stats,
			&format!("{STAT_CORAL} Acc"),
			coral_accuracy,
			"coral_accuracy",
			true
		),
	);
	let page = page.replace(
		"{{algae-score}}",
		stat_card_float!(
			team_stats,
			&format!("{STAT_ALGAE} Sco"),
			algae_score,
			"algae_score",
			true
		),
	);
	let page = page.replace(
		"{{processor-average}}",
		stat_card_float!(
			team_stats,
			"Proc Avg",
			processor_average,
			"processor_average",
			false
		),
	);
	let page = page.replace(
		"{{processor-accuracy}}",
		stat_card_pct!(
			team_stats,
			"Proc Acc",
			processor_accuracy,
			"processor_accuracy",
			false
		),
	);
	let page = page.replace(
		"{{net-average}}",
		stat_card_float!(team_stats, "Net Avg", net_average, "net_average", false),
	);
	let page = page.replace(
		"{{intake-accuracy}}",
		stat_card_pct!(
			team_stats,
			"Intk Acc",
			intake_accuracy,
			"intake_accuracy",
			false
		),
	);
	let page = page.replace(
		"{{offense-average}}",
		stat_card_float!(
			team_stats,
			"Off Avg",
			offense_average,
			"offense_average",
			false
		),
	);
	let page = page.replace(
		"{{defense-average}}",
		stat_card_float!(
			team_stats,
			"Def Avg",
			defense_average,
			"defense_average",
			false
		),
	);
	let page = page.replace(
		"{{time-to-first-cycle}}",
		stat_card_float!(
			team_stats,
			"TTFC",
			time_to_first_cycle,
			"time_to_first_cycle",
			false
		),
	);
	let page = page.replace(
		"{{l1-count}}",
		stat_card_other!(team_stats, "L1 #", l1_count, "l1_count", false),
	);
	let page = page.replace(
		"{{l2-count}}",
		stat_card_other!(team_stats, "L2 #", l2_count, "l2_count", false),
	);
	let page = page.replace(
		"{{l3-count}}",
		stat_card_other!(team_stats, "L3 #", l3_count, "l3_count", false),
	);
	let page = page.replace(
		"{{l4-count}}",
		stat_card_other!(team_stats, "L4 #", l4_count, "l4_count", false),
	);
	let page = page.replace(
		"{{l1-accuracy}}",
		stat_card_pct!(team_stats, "L1 Acc", l1_accuracy, "l1_accuracy", false),
	);
	let page = page.replace(
		"{{l2-accuracy}}",
		stat_card_pct!(team_stats, "L2 Acc", l2_accuracy, "l2_accuracy", false),
	);
	let page = page.replace(
		"{{l3-accuracy}}",
		stat_card_pct!(team_stats, "L3 Acc", l3_accuracy, "l3_accuracy", false),
	);
	let page = page.replace(
		"{{l4-accuracy}}",
		stat_card_pct!(team_stats, "L4 Acc", l4_accuracy, "l4_accuracy", false),
	);
	let page = page.replace(
		"{{l1-value}}",
		stat_card_float!(team_stats, "L1 Value", l1_value, "l1_value", false),
	);
	let page = page.replace(
		"{{l2-value}}",
		stat_card_float!(team_stats, "L2 Value", l2_value, "l2_value", false),
	);
	let page = page.replace(
		"{{l3-value}}",
		stat_card_float!(team_stats, "L3 Value", l3_value, "l3_value", false),
	);
	let page = page.replace(
		"{{l4-value}}",
		stat_card_float!(team_stats, "L4 Value", l4_value, "l4_value", false),
	);
	let page = page.replace(
		"{{climb-accuracy}}",
		stat_card_pct!(
			team_stats,
			"Accuracy",
			climb_accuracy,
			"climb_accuracy",
			true
		),
	);
	let page = page.replace(
		"{{climb-time}}",
		stat_card_float!(team_stats, "Avg Time", climb_time, "climb_time", true),
	);
	let page = page.replace(
		"{{climb-score}}",
		stat_card_float!(team_stats, "Score", climb_score, "climb_score", true),
	);
	let page = page.replace(
		"{{climb-fall-percent}}",
		stat_card_pct!(
			team_stats,
			"Fall Pct",
			climb_fall_percent,
			"climb_fall_percent",
			false
		),
	);

	// Team info
	let team_info = lock
		.get_team_info(team.number)
		.await
		.map_err(|e| {
			error!("Failed to get team info from database: {e}");
			Status::InternalServerError
		})?
		.unwrap_or_default();

	let page = page.replace(
		"{{max-speed}}",
		&render_stat_card_optional_float("Max Speed", "", team_info.max_speed, true, ""),
	);
	let page = page.replace(
		"{{height}}",
		&render_stat_card_optional_float("Height", "", team_info.height, true, ""),
	);
	let page = page.replace(
		"{{weight}}",
		&render_stat_card_optional_float("Weight", "", team_info.weight, true, ""),
	);
	let page = page.replace(
		"{{length}}",
		&render_stat_card_optional_float("Length", "", team_info.length, false, ""),
	);
	let page = page.replace(
		"{{width}}",
		&render_stat_card_optional_float("Width", "", team_info.width, false, ""),
	);
	let page = page.replace(
		"{{drivetrain-type}}",
		&render_stat_card_optional(
			"Drivetrain",
			"",
			team_info.drivetrain_type.map(|x| match x {
				DriveTrainType::Swerve => "Sw",
				DriveTrainType::Tank => "Tk",
				DriveTrainType::Mecanum => "Mc",
				DriveTrainType::Other => "Ot",
			}),
			false,
			"",
		),
	);
	let page = page.replace(
		"{{can-pickup-algae}}",
		&render_stat_card_optional_bool(
			&format!("{STAT_ALGAE} Int?"),
			"",
			team_info.can_pickup_algae,
			false,
			"",
		),
	);
	let page = page.replace(
		"{{can-pickup-coral}}",
		&render_stat_card_optional_bool(
			&format!("{STAT_CORAL} Int?"),
			"",
			team_info.can_pickup_coral,
			false,
			"",
		),
	);
	let page = page.replace(
		"{{can-hold-both}}",
		&render_stat_card_optional_bool("Hold Both?", "", team_info.can_hold_both, false, ""),
	);
	let page = page.replace(
		"{{can-ground-intake-algae}}",
		&render_stat_card_optional_bool(
			&format!("{STAT_ALGAE} Ground?"),
			"",
			team_info.can_ground_intake_algae,
			false,
			"",
		),
	);
	let page = page.replace(
		"{{can-ground-intake-coral}}",
		&render_stat_card_optional_bool(
			&format!("{STAT_CORAL} Ground?"),
			"",
			team_info.can_ground_intake_coral,
			false,
			"",
		),
	);
	let page = page.replace(
		"{{can-slide-intake}}",
		&render_stat_card_optional_bool("Slide Int?", "", team_info.can_slide_intake, false, ""),
	);
	let page = page.replace(
		"{{can-reef}}",
		&render_stat_card_optional_bool("Reef?", "", team_info.can_reef, false, ""),
	);
	let page = page.replace(
		"{{can-processor}}",
		&render_stat_card_optional_bool("Processor?", "", team_info.can_processor, false, ""),
	);
	let page = page.replace(
		"{{can-net}}",
		&render_stat_card_optional_bool("Net?", "", team_info.can_net, false, ""),
	);
	let page = page.replace(
		"{{can-agitate}}",
		&render_stat_card_optional_bool("Agitate?", "", team_info.can_agitate, false, ""),
	);

	let l1_elem = render_reef_level(team_info.can_l1.unwrap_or_default());
	let l2_elem = render_reef_level(team_info.can_l2.unwrap_or_default());
	let l3_elem = render_reef_level(team_info.can_l3.unwrap_or_default());
	let l4_elem = render_reef_level(team_info.can_l4.unwrap_or_default());
	let reef_level =
		format!("<div class=\"reef-ability\">{l1_elem}{l2_elem}{l3_elem}{l4_elem}</div>");
	let page = page.replace(
		"{{reef-ability}}",
		&render_stat_card("Reef Lvls", "", reef_level, true, ""),
	);

	let page = page.replace(
		"{{can-shallow}}",
		&render_stat_card_optional_bool("Shallow?", "", team_info.can_shallow, false, ""),
	);
	let page = page.replace(
		"{{can-deep}}",
		&render_stat_card_optional_bool("Deep?", "", team_info.can_deep, false, ""),
	);

	let page = page.replace(
		"{{preferred-piece}}",
		&render_stat_card_optional(
			"Fave",
			"",
			team_info.preferred_piece.map(|x| match x {
				GamePiece::Algae => {
					"<img src=\"/assets/icons/algae.svg\" style=\"width:1.7rem\" />"
				}
				GamePiece::Coral => {
					"<img src=\"/assets/icons/coral.svg\" style=\"width:1.2rem\" />"
				}
			}),
			false,
			"",
		),
	);
	let page = page.replace(
		"{{pit-cycle-time}}",
		&render_stat_card_optional_float("CT", "", team_info.cycle_time, false, ""),
	);
	let page = page.replace(
		"{{pit-climb-time}}",
		&render_stat_card_optional_float("Clmb Time", "", team_info.climb_time, false, ""),
	);
	let page = page.replace(
		"{{align-score}}",
		&render_stat_card_optional_bool("Score Align?", "", team_info.align_score, false, ""),
	);
	let page = page.replace(
		"{{align-intake}}",
		&render_stat_card_optional_bool("Intk Align?", "", team_info.align_intake, false, ""),
	);
	let page = page.replace(
		"{{auto-crosses-line}}",
		&render_stat_card_optional_bool("Auto Cross?", "", team_info.auto_crosses_line, false, ""),
	);
	let page = page.replace(
		"{{auto-scores-front}}",
		&render_stat_card_optional_bool("Auto Front?", "", team_info.auto_scores_front, false, ""),
	);
	let page = page.replace(
		"{{auto-scores-back}}",
		&render_stat_card_optional_bool("Auto Back?", "", team_info.auto_scores_back, false, ""),
	);
	let page = page.replace(
		"{{auto-scores-side}}",
		&render_stat_card_optional_bool("Auto Side?", "", team_info.auto_scores_side, false, ""),
	);
	let page = page.replace(
		"{{pit-auto-algae}}",
		&render_stat_card_optional(
			&format!("{STAT_ALGAE} Auto"),
			"",
			team_info.auto_algae.map(|x| x.to_string()),
			false,
			"",
		),
	);
	let page = page.replace(
		"{{pit-auto-coral}}",
		&render_stat_card_optional(
			&format!("{STAT_CORAL} Auto"),
			"",
			team_info.auto_coral.map(|x| x.to_string()),
			false,
			"",
		),
	);
	let page = page.replace(
		"{{uses-pathplanner}}",
		&render_stat_card_optional_bool("PP?", "", team_info.uses_pathplanner, false, ""),
	);
	let page = page.replace(
		"{{two-can-networks}}",
		&render_stat_card_optional_bool("2CAN?", "", team_info.two_can_networks, false, ""),
	);

	let page = page.replace("{{notes}}", &team_info.notes);

	// Pit scouting progress
	let page = page.replace("{{pit-scouting-progress}}", &team_info.progress.to_string());
	let page = page.replace("{{pit-scouting-color}}", team_info.progress.get_color());

	let page = create_page("Team Details", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

fn render_reef_level(enabled: bool) -> &'static str {
	if enabled {
		"<div class=\"reef-level selected\"></div>"
	} else {
		"<div class=\"reef-level\"></div>"
	}
}
