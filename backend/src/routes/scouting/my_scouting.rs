use chrono::DateTime;
use chrono_tz::US::Eastern;
use rocket::{
	http::Status,
	response::{content::RawHtml, Redirect},
};
use tracing::{error, span, Level};

use crate::{
	db::Database,
	routes::OptionalSessionID,
	scouting::{
		assignment::ScoutingAssignment,
		matches::{Match, MatchStats},
		TeamNumber,
	},
	util::render_time,
	State,
};

use super::{create_page, PageOrRedirect, Scope};

#[rocket::get("/scouting/my")]
pub async fn my_scouting(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "My scouting");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	let Ok(requesting_member) = session_id.get_requesting_member(state).await else {
		return Ok(redirect);
	};

	let page = include_str!("../pages/scouting/my_scouting.min.html");

	let lock = state.db.lock().await;

	let assignment = lock
		.get_assignment(&requesting_member.id)
		.await
		.map_err(|e| {
			error!("Failed to get assignment for member from database: {e}");
			Status::InternalServerError
		})?
		.unwrap_or(ScoutingAssignment {
			member: requesting_member.id.clone(),
			..Default::default()
		});

	let mut teams_string = String::new();
	for team in &assignment.teams {
		teams_string.push_str(&render_team(*team));
	}
	let page = page.replace("{{teams}}", &teams_string);

	let match_stats = lock.get_all_match_stats().await.map_err(|e| {
		error!("Failed to get all match stats from database: {e}");
		Status::InternalServerError
	})?;
	let match_stats: Vec<_> = match_stats.collect();

	let matches = lock.get_matches().await.map_err(|e| {
		error!("Failed to get all matches from database: {e}");
		Status::InternalServerError
	})?;
	let matches = matches.filter(|x| {
		x.red_alliance.iter().any(|x| assignment.teams.contains(x))
			|| x.blue_alliance.iter().any(|x| assignment.teams.contains(x))
	});

	let mut matches_string = String::new();
	for m in matches {
		matches_string.push_str(&render_match(m, &match_stats, &assignment.teams));
	}
	let page = page.replace("{{matches}}", &matches_string);

	let page = create_page("My Scouting", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

fn render_team(team: TeamNumber) -> String {
	format!("<div class=\"cont round team\">{team}</div>")
}

fn render_match(m: Match, match_stats: &[MatchStats], assigned_teams: &[TeamNumber]) -> String {
	let out = include_str!("../components/scouting/my_match.min.html");

	let out = out.replace("{{number}}", &m.num.num.to_string());

	let date = if let Some(Ok(date)) = m.date.map(|x| DateTime::parse_from_rfc2822(&x)) {
		render_time(date.with_timezone(&Eastern))
	} else {
		String::new()
	};
	let out = out.replace("{{time}}", &date);

	let teams = assigned_teams
		.iter()
		.filter(|x| m.blue_alliance.contains(x) || m.red_alliance.contains(x));
	let mut teams_string = String::new();
	for team in teams {
		teams_string.push_str(&render_team(*team));
	}
	let out = out.replace("{{teams}}", &teams_string);

	let is_done = match_stats.iter().any(|x| {
		x.match_number.as_ref().is_some_and(|x| x == &m.num)
			&& assigned_teams.contains(&x.team_number)
	});
	let is_done_icon = if is_done {
		"<img src=/assets/icons/check.svg />"
	} else {
		""
	};
	let out = out.replace("{{done-icon}}", is_done_icon);

	out
}
