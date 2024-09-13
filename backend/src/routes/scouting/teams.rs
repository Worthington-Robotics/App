use std::fmt::Display;

use itertools::Itertools;
use rocket::{
	form::Form,
	http::Status,
	response::{content::RawHtml, Redirect},
	FromForm,
};
use rocket_async_compression::{Compress, Level as CompressionLevel};
use strum::IntoEnumIterator;
use tracing::{error, span, Level};

use crate::{
	api::statbotics::StatboticsClient,
	db::Database,
	routes::{OptionalSessionID, SessionID},
	scouting::{Competition, DriveTrainType, IntakeType, Team, TeamNumber, TeamStats},
	util::{checkbox_attr, selected_attr},
	State,
};

use super::{create_page, PageOrRedirect, Scope};

#[rocket::get("/scouting/teams?<competition>")]
pub async fn teams(
	session_id: OptionalSessionID<'_>,
	state: &State,
	competition: Option<&str>,
) -> Result<Compress<PageOrRedirect>, Status> {
	let span = span!(Level::DEBUG, "Teams");
	let _enter = span.enter();

	let mut competition = competition.unwrap_or_default();
	// If the competition is "current", replace it with whatever the current competition is
	if competition == "Current" {
		// TODO: Use the actual current competition
		competition = "Pittsburgh";
	}

	let parsed_competition = Competition::from_db(competition);

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(Compress(redirect, CompressionLevel::Fastest));
	};

	if session_id.get_requesting_member(state).await.is_err() {
		return Ok(Compress(redirect, CompressionLevel::Fastest));
	};

	let page = include_str!("../pages/scouting/teams.min.html");

	let lock = state.db.lock().await;
	let teams = lock
		.get_teams()
		.await
		.map_err(|e| {
			error!("Failed to get teams from database: {e}");
			Status::InternalServerError
		})?
		.sorted_by_key(|x| x.number);

	let mut teams_string = String::new();
	for team in teams {
		// Skip teams that aren't at the given competition
		if let Some(competition) = &parsed_competition {
			if !team.competitions.contains(competition) {
				continue;
			}
		}
		teams_string.push_str(&render_team(team, &state.statbotics_client).await);
	}
	let page = page.replace("{{teams}}", &teams_string);

	let mut comps_string = String::new();
	// Loop over all competitions along with the option for all teams and the current competition
	for (data, disp) in Competition::iter()
		.map(|x| (x.into(), x.get_abbr()))
		.chain(std::iter::once(("", "All")))
	{
		let is_selected = if data.is_empty() {
			competition.is_empty()
		} else {
			competition == data
		};
		let selected_class = if is_selected { " selected" } else { "" };
		let additional_class = if data.is_empty() { " all" } else { "" };

		let elem = format!(
			r#"<a href=/scouting/teams?competition={data} class="round cont nolink comp{selected_class}{additional_class}"><button>{disp}</button></a>"#
		);

		comps_string.push_str(&elem);
	}
	let page = page.replace("{{comp-options}}", &comps_string);

	let page = create_page("Teams", &page, Some(Scope::Scouting));

	Ok(Compress(
		PageOrRedirect::Page(RawHtml(page)),
		CompressionLevel::Fastest,
	))
}

async fn render_team(team: Team, stat_client: &StatboticsClient) -> String {
	let out = include_str!("../components/scouting/team_row.min.html");
	let out = out.replace("{{number}}", &team.number.to_string());
	let out = out.replace("{{name}}", &team.sanitized_name());
	let epa = stat_client.get_epa(team.number).await.unwrap_or(0.0);
	let out = out.replace("{{epa}}", &format!("{epa:.2}"));

	out
}

#[rocket::get("/scouting/team/<id>")]
pub async fn team_details(
	id: TeamNumber,
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Team details page");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	let Ok(requesting_member) = session_id.get_requesting_member(state).await else {
		return Ok(redirect);
	};

	let lock = state.db.lock().await;
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

	let page = include_str!("../pages/scouting/team/details.min.html");
	let page = page.replace("{{name}}", &team.name);
	let page = page.replace("{{number}}", &team.number.to_string());
	let page = page.replace("{{rookie-year}}", &team.rookie_year.to_string());

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
			r#"<div class="cont comp-cb"><input type=checkbox {disabled_attr} {checked_attr} data-val={comp} /> {}</div>"#,
			comp.get_abbr()
		);
		checkboxes_string.push_str(&component);
	}
	let page = page.replace("{{comp-checkboxes}}", &checkboxes_string);

	let page = page.replace(
		"{{edit-button}}",
		include_str!("../components/ui/edit.min.html"),
	);

	// Create stats
	let epa = state
		.statbotics_client
		.get_epa(id)
		.await
		.unwrap_or_default();
	let page = page.replace("{{epa}}", &render_stat_card_float("EPA", epa, true));

	let default_stats = TeamStats::default();
	let lock2 = state.team_stats.read().await;
	let team_stats = lock2.get(&id).unwrap_or(&default_stats);
	let page = page.replace(
		"{{apa}}",
		&render_stat_card_float("APA", team_stats.apa, true),
	);
	let page = page.replace(
		"{{win-rate}}",
		&render_stat_card_pct("Win Rate", team_stats.win_rate, true),
	);
	let page = page.replace(
		"{{matches}}",
		&render_stat_card("Matches", team_stats.matches, false),
	);
	let page = page.replace(
		"{{reliability}}",
		&render_stat_card_pct("Reliability", team_stats.reliability, false),
	);
	let page = page.replace(
		"{{penalties}}",
		&render_stat_card("Penalties", team_stats.penalties, false),
	);
	let page = page.replace(
		"{{auto-score}}",
		&render_stat_card_float("Score", team_stats.auto_score, true),
	);
	let page = page.replace(
		"{{auto-accuracy}}",
		&render_stat_card_float("Accuracy", team_stats.auto_accuracy, true),
	);
	let page = page.replace(
		"{{auto-collisions}}",
		&render_stat_card("Collisions", team_stats.auto_collisions, false),
	);
	let page = page.replace(
		"{{cycle-time}}",
		&render_stat_card_float("CT", team_stats.cycle_time, true),
	);
	let page = page.replace(
		"{{cycle-time-consistency}}",
		&render_stat_card_pct("CTC", team_stats.cycle_time_consistency, true),
	);
	let page = page.replace(
		"{{speaker-score}}",
		&render_stat_card_float("Spkr Sco", team_stats.speaker_score, false),
	);
	let page = page.replace(
		"{{amp-score}}",
		&render_stat_card_float("Amp Sco", team_stats.amp_score, false),
	);
	let page = page.replace(
		"{{pass-average}}",
		&render_stat_card_float("Pass Avg", team_stats.pass_average, false),
	);
	let page = page.replace(
		"{{speaker-accuracy}}",
		&render_stat_card_pct("Spkr Acc", team_stats.speaker_accuracy, false),
	);
	let page = page.replace(
		"{{amp-accuracy}}",
		&render_stat_card_pct("Amp Acc", team_stats.amp_accuracy, false),
	);
	let page = page.replace(
		"{{amp-rate}}",
		&render_stat_card_float("Amp Rate", team_stats.amplification_rate, true),
	);
	let page = page.replace(
		"{{amp-power}}",
		&render_stat_card_float("Amp Pwr", team_stats.amplification_power, true),
	);
	let page = page.replace(
		"{{defense-average}}",
		&render_stat_card_float("Def Avg", team_stats.defense_average, false),
	);
	let page = page.replace(
		"{{climb-score}}",
		&render_stat_card_float("Climb Sco", team_stats.climb_score, true),
	);
	let page = page.replace(
		"{{climb-accuracy}}",
		&render_stat_card_pct("Climb Acc", team_stats.climb_accuracy, false),
	);
	let page = page.replace(
		"{{trap-score}}",
		&render_stat_card_float("Trap Sco", team_stats.trap_score, true),
	);
	let page = page.replace(
		"{{trap-accuracy}}",
		&render_stat_card_pct("Trap Acc", team_stats.trap_accuracy, false),
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
		&render_stat_card_optional_float("Max Speed", team_info.max_speed, true),
	);
	let page = page.replace(
		"{{height}}",
		&render_stat_card_optional_float("Height", team_info.height, true),
	);
	let page = page.replace(
		"{{weight}}",
		&render_stat_card_optional_float("Weight", team_info.weight, true),
	);
	let page = page.replace(
		"{{can-speaker}}",
		&render_stat_card_optional_bool("Speaker?", team_info.can_speaker, false),
	);
	let page = page.replace(
		"{{can-amp}}",
		&render_stat_card_optional_bool("Amp?", team_info.can_amp, false),
	);
	let page = page.replace(
		"{{can-climb}}",
		&render_stat_card_optional_bool("Climb?", team_info.can_climb, false),
	);
	let page = page.replace(
		"{{can-trap}}",
		&render_stat_card_optional_bool("Trap?", team_info.can_trap, false),
	);
	let page = page.replace(
		"{{can-pass}}",
		&render_stat_card_optional_bool("Pass?", team_info.can_pass, false),
	);
	let page = page.replace(
		"{{can-drive-under-stage}}",
		&render_stat_card_optional_bool("Under Stage?", team_info.can_drive_under_stage, false),
	);
	let page = page.replace(
		"{{can-ground-intake}}",
		&render_stat_card_optional_bool("Ground?", team_info.can_ground_intake, false),
	);
	let page = page.replace(
		"{{can-source-intake}}",
		&render_stat_card_optional_bool("Source?", team_info.can_source_intake, false),
	);
	let page = page.replace(
		"{{intake-type}}",
		&render_stat_card_optional(
			"Intake",
			team_info.intake_type.map(|x| match x {
				IntakeType::OverBumper => "OB",
				IntakeType::UnderBumper => "UB",
			}),
			false,
		),
	);
	let page = page.replace(
		"{{drivetrain-type}}",
		&render_stat_card_optional(
			"Drivetrain",
			team_info.drivetrain_type.map(|x| match x {
				DriveTrainType::Swerve => "S",
				DriveTrainType::Tank => "T",
				DriveTrainType::Mecanum => "M",
				DriveTrainType::Other => "O",
			}),
			false,
		),
	);
	let page = page.replace("{{notes}}", &team_info.notes);

	let page = create_page("Team Details", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

fn render_stat_card(title: &str, stat: impl Display, strong: bool) -> String {
	let out = include_str!("../components/scouting/stat_card.min.html");
	let out = out.replace("{{stat}}", &stat.to_string());
	let out = out.replace("{{title}}", title);
	let class = if strong { "strong" } else { "" };
	let out = out.replace("{{stat-class}}", class);

	out
}

fn render_stat_card_float(title: &str, stat: f32, strong: bool) -> String {
	render_stat_card(title, format!("{stat:.2}"), strong)
}

fn render_stat_card_pct(title: &str, stat: f32, strong: bool) -> String {
	render_stat_card(title, format!("{:.1}%", stat * 100.0), strong)
}

fn render_stat_card_optional(title: &str, stat: Option<impl Display>, strong: bool) -> String {
	if let Some(stat) = stat {
		render_stat_card(title, stat, strong)
	} else {
		render_stat_card(title, "?", strong)
	}
}

fn render_stat_card_optional_bool(title: &str, stat: Option<bool>, strong: bool) -> String {
	if let Some(stat) = stat {
		render_stat_card(title, if stat { "Yes" } else { "No" }, strong)
	} else {
		render_stat_card(title, "?", strong)
	}
}

fn render_stat_card_optional_float(title: &str, stat: Option<f32>, strong: bool) -> String {
	if let Some(stat) = stat {
		render_stat_card_float(title, stat, strong)
	} else {
		render_stat_card(title, "?", strong)
	}
}

#[rocket::get("/scouting/team/<team>/edit_info")]
pub async fn team_info_page(
	session_id: OptionalSessionID<'_>,
	state: &State,
	team: TeamNumber,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Team info editing page");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.get_requesting_member(state).await.is_err() {
		return Ok(redirect);
	};

	let lock = state.db.lock().await;
	let team_info = lock
		.get_team_info(team)
		.await
		.map_err(|e| {
			error!("Failed to get team info from database: {e}");
			Status::InternalServerError
		})?
		.unwrap_or_default();

	let page = include_str!("../pages/scouting/team/info.min.html");
	let page = page.replace("{{team-number}}", &team.to_string());
	let page = page.replace(
		"{{max-speed}}",
		&team_info
			.max_speed
			.map(|x| x.to_string())
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{height}}",
		&team_info.height.map(|x| x.to_string()).unwrap_or_default(),
	);
	let page = page.replace(
		"{{weight}}",
		&team_info.weight.map(|x| x.to_string()).unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-speaker-checked}}",
		&team_info.can_speaker.map(checkbox_attr).unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-amp-checked}}",
		&team_info.can_amp.map(checkbox_attr).unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-climb-checked}}",
		&team_info.can_climb.map(checkbox_attr).unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-trap-checked}}",
		&team_info.can_trap.map(checkbox_attr).unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-pass-checked}}",
		&team_info.can_pass.map(checkbox_attr).unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-drive-under-stage-checked}}",
		&team_info
			.can_drive_under_stage
			.map(checkbox_attr)
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-ground-intake-checked}}",
		&team_info
			.can_ground_intake
			.map(checkbox_attr)
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-source-intake-checked}}",
		&team_info
			.can_source_intake
			.map(checkbox_attr)
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{under-bumper-selected}}",
		selected_attr(
			team_info
				.intake_type
				.is_some_and(|x| x == IntakeType::UnderBumper),
		),
	);
	let page = page.replace(
		"{{over-bumper-selected}}",
		selected_attr(
			team_info
				.intake_type
				.is_some_and(|x| x == IntakeType::OverBumper),
		),
	);
	let page = page.replace(
		"{{swerve-selected}}",
		selected_attr(
			team_info
				.drivetrain_type
				.is_some_and(|x| x == DriveTrainType::Swerve),
		),
	);
	let page = page.replace(
		"{{tank-selected}}",
		selected_attr(
			team_info
				.drivetrain_type
				.is_some_and(|x| x == DriveTrainType::Tank),
		),
	);
	let page = page.replace(
		"{{mecanum-selected}}",
		selected_attr(
			team_info
				.drivetrain_type
				.is_some_and(|x| x == DriveTrainType::Mecanum),
		),
	);
	let page = page.replace(
		"{{drive-other-selected}}",
		selected_attr(
			team_info
				.drivetrain_type
				.is_some_and(|x| x == DriveTrainType::Other),
		),
	);
	let page = page.replace("{{notes}}", &team_info.notes);

	let page = create_page("Edit Team Info", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

#[rocket::post("/api/create_team_info", data = "<info>")]
pub async fn create_team_info(
	state: &State,
	session_id: SessionID<'_>,
	info: Form<TeamInfoForm>,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Creating team info");
	let _enter = span.enter();

	session_id.get_requesting_member(state).await?;

	let team = info.team;
	let info = serde_json::from_str(&info.data).map_err(|e| {
		error!("Invalid team info data: {e}");
		Status::BadRequest
	})?;

	let mut lock = state.db.lock().await;

	if let Err(e) = lock.create_team_info(team, info).await {
		error!("Failed to create team info in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

#[derive(FromForm)]
pub struct TeamInfoForm {
	team: TeamNumber,
	data: String,
}

#[rocket::post("/api/update_team_competition/<id>?<competition>")]
pub async fn update_team_competition(
	state: &State,
	session_id: SessionID<'_>,
	id: TeamNumber,
	competition: String,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Updating team competition");
	let _enter = span.enter();

	session_id.verify_elevated(state).await?;

	let mut lock = state.db.lock().await;
	let Some(mut team) = lock.get_team(id).await.map_err(|e| {
		error!("Failed to get team from database: {e}");
		Status::InternalServerError
	})?
	else {
		error!("Team {id} does not exist");
		return Err(Status::NotFound);
	};

	let Some(competition) = Competition::from_db(&competition) else {
		error!("Unknown competition {competition}");
		return Err(Status::BadRequest);
	};
	if team.competitions.contains(&competition) {
		team.competitions.remove(&competition);
	} else {
		team.competitions.insert(competition);
	}

	if let Err(e) = lock.create_team(team).await {
		error!("Failed to update team {id} in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}
