use rocket::{
	http::Status,
	response::{content::RawHtml, Redirect},
};
use rocket_async_compression::{Compress, Level as CompressionLevel};
use strum::IntoEnumIterator;
use tracing::{error, span, Level};

use crate::{
	db::Database,
	routes::OptionalSessionID,
	scouting::{
		game::ClimbAbility, stats::CombinedTeamStats, status::RobotStatus, Competition,
		DriveTrainType, IntakeType, TeamNumber,
	},
	State,
};

use crate::routes::scouting::{
	create_page,
	stats::{
		render_stat_card_float, render_stat_card_optional, render_stat_card_optional_bool,
		render_stat_card_optional_float, stat_card_float, stat_card_other, stat_card_pct,
		STAT_FUEL,
	},
	PageOrRedirect, Scope,
};

#[rocket::get("/scouting/team/<id>?<competition>")]
pub async fn team_details(
	id: TeamNumber,
	session_id: OptionalSessionID<'_>,
	state: &State,
	competition: Option<&str>,
) -> Result<Compress<PageOrRedirect>, Status> {
	let span = span!(Level::DEBUG, "Team details page");
	let _enter = span.enter();

	let competition_str = competition.unwrap_or("Current");

	let redirect = Compress(
		PageOrRedirect::Redirect(Redirect::to("/login")),
		CompressionLevel::Fastest,
	);
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

	// Overall
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
		"{{total-points}}",
		stat_card_other!(
			team_stats,
			"Total Points",
			total_points,
			"total_points",
			false
		),
	);
	let page = page.replace(
		"{{ranking-points}}",
		stat_card_float!(team_stats, "RP", ranking_points, "ranking_points", true),
	);
	let page = page.replace(
		"{{fuel-rp}}",
		stat_card_float!(team_stats, "Fuel RP", fuel_rp, "fuel_rp", false),
	);
	let page = page.replace(
		"{{climb-rp}}",
		stat_card_float!(team_stats, "Climb RP", climb_rp, "climb_rp", false),
	);
	let page = page.replace(
		"{{total-fuel}}",
		stat_card_other!(team_stats, "Total Fuel", total_fuel, "total_fuel", false),
	);
	let page = page.replace(
		"{{high-score}}",
		stat_card_other!(team_stats, "High Score", high_score, "high_score", false),
	);

	// Auto
	let page = page.replace(
		"{{auto-score}}",
		stat_card_float!(team_stats, "Score", auto_score, "auto_score", true),
	);
	let page = page.replace(
		"{{auto-fuel}}",
		stat_card_float!(team_stats, STAT_FUEL, auto_fuel, "auto_fuel", true),
	);
	let page = page.replace(
		"{{auto-fuel-accuracy}}",
		stat_card_pct!(
			team_stats,
			&format!("{STAT_FUEL} Acc"),
			auto_fuel_accuracy,
			"auto_fuel_accuracy",
			false
		),
	);
	let page = page.replace(
		"{{auto-climb-accuracy}}",
		stat_card_pct!(
			team_stats,
			&format!("Climb Acc"),
			auto_climb_accuracy,
			"auto_climb_accuracy",
			true
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

	// Teleop
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
		"{{active-efficiency}}",
		stat_card_pct!(
			team_stats,
			"Active Eff",
			active_efficiency,
			"active_efficiency",
			false
		),
	);
	let page = page.replace(
		"{{inactive-efficiency}}",
		stat_card_pct!(
			team_stats,
			"Inactive Eff",
			inactive_efficiency,
			"inactive_efficiency",
			false
		),
	);
	let page = page.replace(
		"{{fuel-score}}",
		stat_card_float!(
			team_stats,
			&format!("{STAT_FUEL} Sco"),
			fuel_score,
			"fuel_score",
			true
		),
	);
	let page = page.replace(
		"{{fuel-accuracy}}",
		stat_card_pct!(
			team_stats,
			&format!("{STAT_FUEL} Acc"),
			fuel_accuracy,
			"fuel_accuracy",
			true
		),
	);
	let page = page.replace(
		"{{intake-speed}}",
		stat_card_float!(team_stats, "Itk Speed", intake_speed, "intake_speed", false),
	);
	let page = page.replace(
		"{{fuel-per-intake}}",
		stat_card_float!(team_stats, "FPI", fuel_per_intake, "fuel_per_intake", false),
	);
	let page = page.replace(
		"{{pass-average}}",
		stat_card_float!(team_stats, "Pass Avg", pass_average, "pass_average", false),
	);
	let page = page.replace(
		"{{fuel-per-pass}}",
		stat_card_float!(team_stats, "FPP", fuel_per_pass, "fuel_per_pass", false),
	);

	// Climb
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
		"{{intake-type}}",
		&render_stat_card_optional(
			"Intake",
			"",
			team_info.intake_type.map(|x| match x {
				IntakeType::OverBumper => "OtB",
				IntakeType::UnderBumper => "UtB",
			}),
			false,
			"",
		),
	);
	let page = page.replace(
		"{{can-pass-trench}}",
		&render_stat_card_optional_bool("Under Trench?", "", team_info.can_pass_trench, false, ""),
	);
	let page = page.replace(
		"{{can-pass-bump}}",
		&render_stat_card_optional_bool("Over Bump?", "", team_info.can_pass_bump, false, ""),
	);
	let page = page.replace(
		"{{can-ground-intake}}",
		&render_stat_card_optional_bool("Ground Intk?", "", team_info.can_ground_intake, false, ""),
	);
	let page = page.replace(
		"{{can-station-intake}}",
		&render_stat_card_optional_bool(
			"Station Intk?",
			"",
			team_info.can_station_intake,
			false,
			"",
		),
	);
	let page = page.replace(
		"{{can-score-close}}",
		&render_stat_card_optional_bool("Score Close?", "", team_info.can_score_close, false, ""),
	);
	let page = page.replace(
		"{{can-score-far}}",
		&render_stat_card_optional_bool("Score Far?", "", team_info.can_score_far, false, ""),
	);
	let page = page.replace(
		"{{can-climb-auto}}",
		&render_stat_card_optional_bool("Auto Climb?", "", team_info.can_climb_auto, false, ""),
	);
	let page = page.replace(
		"{{auto-fuel}}",
		&render_stat_card_optional(
			&format!("Auto {STAT_FUEL}"),
			"",
			team_info.auto_fuel,
			false,
			"",
		),
	);
	let page = page.replace(
		"{{fuel-per-shift}}",
		&render_stat_card_optional(
			&format!("{STAT_FUEL} Per Shift"),
			"",
			team_info.fuel_per_shift,
			false,
			"",
		),
	);
	let page = page.replace(
		"{{fuel-storage}}",
		&render_stat_card_optional(
			&format!("{STAT_FUEL} Storage"),
			"",
			team_info.fuel_storage,
			false,
			"",
		),
	);
	let page = page.replace(
		"{{climb-ability}}",
		&render_stat_card_optional(
			"Climb",
			"",
			team_info.climb_ability.map(|x| match x {
				ClimbAbility::None => "None",
				ClimbAbility::L1 => "L1",
				ClimbAbility::L2 => "L2",
				ClimbAbility::L3 => "L3",
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

	Ok(Compress(
		PageOrRedirect::Page(RawHtml(page)),
		CompressionLevel::Fastest,
	))
}
