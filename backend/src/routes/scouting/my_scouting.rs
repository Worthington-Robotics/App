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
		assignment::{MatchClaims, ScoutingAssignment},
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
		.get_prescouting_assignment(&requesting_member.id)
		.await
		.map_err(|e| {
			error!("Failed to get prescouting assignment for member from database: {e}");
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

	let claims = lock.get_all_match_claims().await.map_err(|e| {
		error!("Failed to get all match claims from database: {e}");
		Status::InternalServerError
	})?;
	let claims: Vec<_> = claims.collect();

	let mut matches_string = String::new();
	for m in matches {
		matches_string.push_str(&render_match(
			m,
			&claims,
			&match_stats,
			&requesting_member.id,
		));
	}
	let page = page.replace("{{matches}}", &matches_string);

	let page = create_page("My Scouting", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

fn render_match(
	m: Match,
	claims: &[MatchClaims],
	match_stats: &[MatchStats],
	requesting_member: &str,
) -> String {
	let out = include_str!("../components/scouting/my_match.min.html");

	let default_claims = MatchClaims::default();
	let claims = claims
		.iter()
		.find(|x| x.m == m.num)
		.unwrap_or(&default_claims);

	let out = out.replace("{{number}}", &m.num.num.to_string());

	let date = if let Some(Ok(date)) = m.date.as_ref().map(|x| DateTime::parse_from_rfc2822(x)) {
		render_time(date.with_timezone(&Eastern))
	} else {
		String::new()
	};
	let out = out.replace("{{time}}", &date);

	let mut claim_bubbles_str = String::new();
	claim_bubbles_str.push_str(&render_claim_bubble(
		claims.red_1.as_deref(),
		BubbleStyle::Red,
	));
	claim_bubbles_str.push_str(&render_claim_bubble(
		claims.red_2.as_deref(),
		BubbleStyle::Red,
	));
	claim_bubbles_str.push_str(&render_claim_bubble(
		claims.red_3.as_deref(),
		BubbleStyle::Red,
	));
	claim_bubbles_str.push_str(&render_claim_bubble(
		claims.blue_1.as_deref(),
		BubbleStyle::Blue,
	));
	claim_bubbles_str.push_str(&render_claim_bubble(
		claims.blue_2.as_deref(),
		BubbleStyle::Blue,
	));
	claim_bubbles_str.push_str(&render_claim_bubble(
		claims.blue_3.as_deref(),
		BubbleStyle::Blue,
	));
	let out = out.replace("{{claims}}", &claim_bubbles_str);

	let is_claimed = claims
		.red_1
		.as_ref()
		.is_some_and(|x| x == requesting_member)
		|| claims
			.red_2
			.as_ref()
			.is_some_and(|x| x == requesting_member)
		|| claims
			.red_3
			.as_ref()
			.is_some_and(|x| x == requesting_member)
		|| claims
			.blue_1
			.as_ref()
			.is_some_and(|x| x == requesting_member)
		|| claims
			.blue_2
			.as_ref()
			.is_some_and(|x| x == requesting_member)
		|| claims
			.blue_3
			.as_ref()
			.is_some_and(|x| x == requesting_member);

	if !is_claimed && claims.is_full() {
		return String::new();
	}

	let class = if is_claimed { "claimed" } else { "available" };
	let out = out.replace("{{class}}", class);

	// let is_done = match_stats.iter().any(|x| {
	// 	x.match_number.as_ref().is_some_and(|x| x == &m.num)
	// 		&& assigned_teams.contains(&x.team_number)
	// });
	let is_done = false;
	let is_done_icon = if is_done {
		"<img src=/assets/icons/check.svg />"
	} else {
		""
	};
	let out = out.replace("{{done-icon}}", is_done_icon);

	out
}

/// Render one of the little team bubbles to show claim status
fn render_claim_bubble(claim: Option<&str>, style: BubbleStyle) -> String {
	let class = if claim.is_some() {
		match style {
			BubbleStyle::Red => " r",
			BubbleStyle::Blue => " b",
		}
	} else {
		""
	};
	format!("<div class=\"claim{class}\"></div>")
}

/// Style for one of the team bubbles in a match
enum BubbleStyle {
	Red,
	Blue,
}

fn render_team(team: TeamNumber) -> String {
	format!("<div class=\"cont round team\">{team}</div>")
}
