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
	scouting::{game::GamePiece, DriveTrainType, PitScoutingProgress, TeamNumber},
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

	let lock = state.db.read().await;
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
		"{{can-pickup-algae-checked}}",
		&team_info
			.can_pickup_algae
			.map(checkbox_attr)
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-pickup-coral-checked}}",
		&team_info
			.can_pickup_coral
			.map(checkbox_attr)
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-hold-both-checked}}",
		&team_info
			.can_hold_both
			.map(checkbox_attr)
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-ground-intake-algae-checked}}",
		&team_info
			.can_ground_intake_algae
			.map(checkbox_attr)
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-ground-intake-coral-checked}}",
		&team_info
			.can_ground_intake_coral
			.map(checkbox_attr)
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-slide-intake-checked}}",
		&team_info
			.can_slide_intake
			.map(checkbox_attr)
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-reef-checked}}",
		&team_info.can_reef.map(checkbox_attr).unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-processor-checked}}",
		&team_info
			.can_processor
			.map(checkbox_attr)
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-net-checked}}",
		&team_info.can_net.map(checkbox_attr).unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-agitate-checked}}",
		&team_info.can_agitate.map(checkbox_attr).unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-l1-checked}}",
		&team_info.can_l1.map(checkbox_attr).unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-l2-checked}}",
		&team_info.can_l2.map(checkbox_attr).unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-l3-checked}}",
		&team_info.can_l3.map(checkbox_attr).unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-l4-checked}}",
		&team_info.can_l4.map(checkbox_attr).unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-shallow-checked}}",
		&team_info.can_shallow.map(checkbox_attr).unwrap_or_default(),
	);
	let page = page.replace(
		"{{can-deep-checked}}",
		&team_info.can_deep.map(checkbox_attr).unwrap_or_default(),
	);
	let page = page.replace(
		"{{piece-coral-selected}}",
		selected_attr(
			team_info
				.preferred_piece
				.is_some_and(|x| x == GamePiece::Coral),
		),
	);
	let page = page.replace(
		"{{piece-algae-selected}}",
		selected_attr(
			team_info
				.preferred_piece
				.is_some_and(|x| x == GamePiece::Algae),
		),
	);
	let page = page.replace(
		"{{cycle-time}}",
		&team_info
			.cycle_time
			.map(|x| x.to_string())
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{climb-time}}",
		&team_info
			.climb_time
			.map(|x| x.to_string())
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{align-score-checked}}",
		&team_info.align_score.map(checkbox_attr).unwrap_or_default(),
	);
	let page = page.replace(
		"{{align-intake-checked}}",
		&team_info
			.align_intake
			.map(checkbox_attr)
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{auto-crosses-line-checked}}",
		&team_info
			.auto_crosses_line
			.map(checkbox_attr)
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{auto-scores-front-checked}}",
		&team_info
			.auto_scores_front
			.map(checkbox_attr)
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{auto-scores-back-checked}}",
		&team_info
			.auto_scores_back
			.map(checkbox_attr)
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{auto-scores-side-checked}}",
		&team_info
			.auto_scores_side
			.map(checkbox_attr)
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{auto-algae}}",
		&team_info
			.auto_algae
			.map(|x| x.to_string())
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{auto-coral}}",
		&team_info
			.auto_coral
			.map(|x| x.to_string())
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{uses-pathplanner-checked}}",
		&team_info
			.uses_pathplanner
			.map(checkbox_attr)
			.unwrap_or_default(),
	);
	let page = page.replace(
		"{{two-can-networks-checked}}",
		&team_info
			.two_can_networks
			.map(checkbox_attr)
			.unwrap_or_default(),
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

	let mut lock = state.db.write().await;

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

#[rocket::post("/api/scouting/reset_team_info")]
pub async fn reset_team_info(state: &State, session_id: SessionID<'_>) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Resetting all team info");
	let _enter = span.enter();

	session_id.verify_elevated(state).await?;

	let mut lock = state.db.write().await;
	let infos: Vec<_> = lock
		.get_all_team_info()
		.await
		.map_err(|e| {
			error!("Failed to get all team info from database: {e}");
			Status::InternalServerError
		})?
		.collect();

	for (team, mut info) in infos {
		// Move already scouted teams to needing update, and keep non-scouted teams the same
		if info.progress == PitScoutingProgress::Finished {
			info.progress = PitScoutingProgress::NeedsRefresh;

			if let Err(e) = lock.create_team_info(team, info).await {
				error!("Failed to create team info in database: {e}");
			}
		}
	}

	Ok(())
}
