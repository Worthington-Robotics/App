use std::cmp::Reverse;

use chrono::DateTime;
use itertools::Itertools;
use rocket::{
	http::Status,
	response::{content::RawHtml, Redirect},
};
use tracing::{error, span, Level};

use crate::{
	db::Database,
	routes::{create_page, OptionalSessionID, PageOrRedirect, Scope},
	scouting::{
		matches::{MatchStats, MatchStatsID},
		Competition, TeamNumber,
	},
	util::ToDropdown,
	State,
};

#[rocket::get("/scouting/review?<team>&<competition>")]
pub async fn match_review(
	session_id: OptionalSessionID<'_>,
	state: &State,
	team: Option<TeamNumber>,
	competition: Option<&str>,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Match review");
	let _enter = span.enter();

	let competition = competition.and_then(|x| Competition::from_db(x));

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.get_requesting_member(state).await.is_err() {
		return Ok(redirect);
	};

	let page = include_str!("../../pages/scouting/match_review.min.html");

	let lock = state.db.read().await;
	let match_stats = lock.get_all_match_stats().await.map_err(|e| {
		error!("Failed to get match stats from database: {e}");
		Status::InternalServerError
	})?;

	let match_stats = match_stats.filter(|x| {
		if let Some(team) = team {
			return x.team_number == team;
		}

		if let Some(competition) = competition {
			return x.competition == Some(competition);
		}

		true
	});

	let match_stats = match_stats.sorted_by_cached_key(|x| {
		Reverse(
			x.record_time
				.as_ref()
				.and_then(|x| DateTime::parse_from_rfc2822(&x).ok())
				.unwrap_or_default(),
		)
	});

	let mut matches_string = String::new();

	for m in match_stats {
		matches_string.push_str(&render_match_stats(m).await);
	}
	let page = page.replace("{{matches}}", &matches_string);

	let subtitle = if let Some(competition) = competition {
		competition.to_string()
	} else if let Some(team) = team {
		team.to_string()
	} else {
		"Matches".into()
	};
	let page = page.replace("{{subtitle}}", &subtitle);

	// Add a competition dropdown if we need it
	let competition_dropdown = if team.is_none() {
		let options = Competition::create_options(competition.as_ref());
		let options = format!("<option value=none>All</option>{options}");

		format!("<select id=competition>{options}</select>")
	} else {
		String::new()
	};
	let page = page.replace("{{competition-dropdown}}", &competition_dropdown);

	let page = create_page("Match Review", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

async fn render_match_stats(m: MatchStats) -> String {
	let out = include_str!("../../components/scouting/match_stats.min.html");
	let out = out.replace("{{stats-id}}", &m.get_id().to_string());
	let out = out.replace(
		"{{number}}",
		&m.match_number.map(|x| x.to_string()).unwrap_or_default(),
	);
	let out = out.replace(
		"{{competition}}",
		&m.competition.map(|x| x.get_abbr()).unwrap_or_default(),
	);
	let out = out.replace("{{recorder}}", &m.recorder.unwrap_or_default());
	let out = out.replace("{{team-number}}", &m.team_number.to_string());

	let notes = m.notes.replace("<", "");
	let notes = notes.replace(">", "");
	let notes = notes.replace("/", "");
	let out = out.replace("{{notes}}", &notes);

	let border_color = if m.won {
		"var(--wbblue)"
	} else {
		"var(--wbred)"
	};
	let out = out.replace("{{border-color}}", border_color);

	out
}

#[rocket::get("/scouting/edit_match/<stats_id>")]
pub async fn edit_match(
	session_id: OptionalSessionID<'_>,
	state: &State,
	stats_id: &str,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Editing match");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.get_requesting_member(state).await.is_err() {
		return Ok(redirect);
	};

	let lock = state.db.read().await;
	let Some(m) = lock
		.get_match_stats(&MatchStatsID::from_str(stats_id.to_string()))
		.await
		.map_err(|e| {
			error!("Failed to get match stats from database: {e}");
			Status::InternalServerError
		})?
	else {
		error!("Tried to get non-existent match stats {stats_id}");
		return Err(Status::NotFound);
	};

	let page = include_str!("../../pages/scouting/edit_match.min.html");

	let Ok(match_data) = serde_json::to_string_pretty(&m) else {
		error!("Failed to serialize match data");
		return Err(Status::InternalServerError);
	};
	let page = page.replace("{{match-json}}", &match_data);

	let page = page.replace("{{team}}", &m.team_number.to_string());
	let page = page.replace(
		"{{match-number}}",
		&m.match_number.map(|x| x.to_string()).unwrap_or_default(),
	);
	let page = page.replace(
		"{{competition}}",
		&m.competition
			.map(|x| x.to_string())
			.unwrap_or("Unknown".into()),
	);
	let page = page.replace("{{stats-id}}", stats_id);

	let page = create_page("Match Review", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}
