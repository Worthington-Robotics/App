use std::collections::HashMap;

use chrono::{DateTime, Utc};
use itertools::Itertools;
use rocket::{
	form::Form,
	http::Status,
	response::{content::RawHtml, Redirect},
	FromForm,
};
use tracing::{error, span, Level};

use crate::{
	db::Database,
	routes::OptionalSessionID,
	scouting::{
		status::{RobotStatus, StatusReason, StatusUpdate},
		TeamNumber,
	},
	util::{render_date, TIMEZONE},
	State,
};

use super::{create_page, PageOrRedirect, Scope};

#[rocket::get("/scouting/team/<team>/status")]
pub async fn team_status_page(
	session_id: OptionalSessionID<'_>,
	state: &State,
	team: TeamNumber,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Team status page");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.get_requesting_member(state).await.is_err() {
		return Ok(redirect);
	};

	let lock = state.db.read().await;
	let updates = lock.get_team_status(team).await.map_err(|e| {
		error!("Failed to get team status updates from database: {e}");
		Status::InternalServerError
	})?;

	let page = include_str!("../pages/scouting/team/status.min.html");
	let page = page.replace("{{number}}", &team.to_string());

	let mut updates_string = String::new();
	let mut all_reasons = HashMap::new();
	for update in updates.into_iter().rev() {
		updates_string.push_str(&render_status_update(update, &mut all_reasons));
	}
	let page = page.replace("{{updates}}", &updates_string);

	// Add reason counts to the top of the page
	let mut reasons_string = String::new();
	for (reason, count) in all_reasons.into_iter().sorted_by_key(|x| x.1).rev() {
		reasons_string.push_str(&render_reason(&format!("{reason} x {count}")));
	}
	let page = page.replace("{{reasons}}", &reasons_string);

	let page = create_page("Robot Status", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

fn render_status_update(
	update: StatusUpdate,
	all_reasons: &mut HashMap<StatusReason, u8>,
) -> String {
	let out = include_str!("../components/scouting/status_update.min.html");
	let date = if let Ok(date) = DateTime::parse_from_rfc2822(&update.date) {
		render_date(date.with_timezone(TIMEZONE))
	} else {
		String::from("Invalid Date")
	};
	let out = out.replace("{{date}}", &date);
	let out = out.replace("{{details}}", &update.details);
	let out = out.replace("{{status}}", &update.status.to_string());
	let out = out.replace("{{status-color}}", update.status.get_color());
	let out = out.replace(
		"{{competition}}",
		&update
			.competition
			.map(|x| x.to_string())
			.unwrap_or_default(),
	);

	let reasons = update.infer_reasons();
	let mut reasons_string = String::new();
	for reason in reasons {
		reasons_string.push_str(&render_reason(&reason.to_string()));
		// Don't add reasons for good status to the totals
		if update.status != RobotStatus::Good {
			*all_reasons.entry(reason).or_default() += 1;
		}
	}
	let out = out.replace("{{reasons}}", &reasons_string);

	out
}

fn render_reason(text: &str) -> String {
	format!("<div class=\"cont round reason\">{text}</div>")
}

#[rocket::post("/api/update_team_status", data = "<status>")]
pub async fn update_status(
	state: &State,
	session_id: OptionalSessionID<'_>,
	status: Form<StatusUpdateForm>,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Creating status update");
	let _enter = span.enter();

	let Some(session_id) = session_id.to_session_id() else {
		return Err(Status::Unauthorized);
	};
	let requesting_member = session_id.get_requesting_member(state).await?;

	let date = Utc::now().to_rfc2822();

	let mut lock = state.db.write().await;

	let global_data = lock.get_global_data().await.map_err(|e| {
		error!("Failed to get global data from database: {e}");
		Status::InternalServerError
	})?;

	let update = StatusUpdate {
		team: status.team,
		member: requesting_member.id,
		details: status.details.clone(),
		status: status.status,
		date,
		competition: global_data.current_competition,
	};

	if let Err(e) = lock.update_team_status(update).await {
		error!("Failed to create status update in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

#[derive(FromForm)]
pub struct StatusUpdateForm {
	team: TeamNumber,
	status: RobotStatus,
	details: String,
}
