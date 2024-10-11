use rocket::{
	form::Form,
	http::Status,
	response::{content::RawHtml, Redirect},
	FromForm,
};
use tracing::{error, span, Level};

use crate::{
	db::Database,
	routes::{OptionalSessionID, SessionID},
	scouting::{DriveTrainType, IntakeType, PitScoutingProgress, TeamNumber},
	util::{checkbox_attr, selected_attr},
	State,
};

use super::{create_page, PageOrRedirect, Scope};

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

	let page = include_str!("../../pages/scouting/team/info.min.html");
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
		"{{length}}",
		&team_info.length.map(|x| x.to_string()).unwrap_or_default(),
	);
	let page = page.replace(
		"{{width}}",
		&team_info.width.map(|x| x.to_string()).unwrap_or_default(),
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

	let page = page.replace(
		"{{needs-refresh-selected}}",
		selected_attr(team_info.progress == PitScoutingProgress::NeedsRefresh),
	);
	let page = page.replace(
		"{{finished-selected}}",
		selected_attr(team_info.progress == PitScoutingProgress::Finished),
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
