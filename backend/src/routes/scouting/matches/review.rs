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
	scouting::{matches::MatchStats, TeamNumber},
	State,
};

#[rocket::get("/scouting/review/<team>")]
pub async fn match_review(
	session_id: OptionalSessionID<'_>,
	state: &State,
	team: TeamNumber,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Match review");
	let _enter = span.enter();

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

	let match_stats = match_stats.filter(|x| x.team_number == team);

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

	let page = page.replace("{{team}}", &team.to_string());

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

	let notes = m.notes.replace("<", "");
	let notes = notes.replace(">", "");
	let notes = notes.replace("/", "");
	let out = out.replace("{{notes}}", &notes);

	out
}
