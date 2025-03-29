use chrono::DateTime;
use itertools::Itertools;
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
		matches::{count_matches_scouted, Match, MatchStats},
		TeamNumber,
	},
	util::{render_time, TIMEZONE},
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

	let lock = state.db.read().await;

	// Assignments
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
		teams_string.push_str(&render_team(*team, TeamStyle::None));
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
	for m in matches.sorted_by_key(|x| x.num.num) {
		matches_string.push_str(&render_match(
			m,
			&claims,
			&match_stats,
			&requesting_member.id,
		));
	}
	let page = page.replace("{{matches}}", &matches_string);

	// Scouting progress
	let global_data = lock.get_global_data().await.map_err(|e| {
		error!("Failed to get global data from database: {e}");
		Status::InternalServerError
	})?;

	let match_stats = lock.get_all_match_stats().await.map_err(|e| {
		error!("Failed to get match stats from database: {e}");
		Status::InternalServerError
	})?;
	let match_stats: Vec<_> = match_stats.collect();

	let matches_scouted = count_matches_scouted(
		&requesting_member.id,
		&match_stats,
		global_data.current_competition.as_ref(),
	);
	let mut completion_amount = matches_scouted as f32 / 30.0;
	if completion_amount > 1.0 {
		completion_amount = 1.0;
	}
	let page = page.replace("{{completed-matches}}", &matches_scouted.to_string());
	let page = page.replace(
		"{{progress-width}}",
		&format!("{:.0}", completion_amount * 100.0),
	);

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
		render_time(date.with_timezone(TIMEZONE))
	} else {
		String::new()
	};
	let out = out.replace("{{time}}", &date);

	let mut claim_bubbles_str = String::new();
	claim_bubbles_str.push_str(&render_claim_bubble(
		claims.red_1.as_deref(),
		AllianceStyle::Red,
	));
	claim_bubbles_str.push_str(&render_claim_bubble(
		claims.red_2.as_deref(),
		AllianceStyle::Red,
	));
	claim_bubbles_str.push_str(&render_claim_bubble(
		claims.red_3.as_deref(),
		AllianceStyle::Red,
	));
	claim_bubbles_str.push_str(&render_claim_bubble(
		claims.blue_1.as_deref(),
		AllianceStyle::Blue,
	));
	claim_bubbles_str.push_str(&render_claim_bubble(
		claims.blue_2.as_deref(),
		AllianceStyle::Blue,
	));
	claim_bubbles_str.push_str(&render_claim_bubble(
		claims.blue_3.as_deref(),
		AllianceStyle::Blue,
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

	// Figure out which team the member has claimed
	let claimed_slot = &[
		&claims.red_1,
		&claims.red_2,
		&claims.red_3,
		&claims.blue_1,
		&claims.blue_2,
		&claims.blue_3,
	]
	.into_iter()
	.position(|x| x.as_ref().is_some_and(|x| x == requesting_member));
	let claimed_team = match claimed_slot {
		Some(0) => Some(m.red_alliance[0]),
		Some(1) => Some(m.red_alliance[1]),
		Some(2) => Some(m.red_alliance[2]),
		Some(3) => Some(m.blue_alliance[0]),
		Some(4) => Some(m.blue_alliance[1]),
		Some(5) => Some(m.blue_alliance[2]),
		Some(_) | None => None,
	};

	let claimed_team_elem = if let Some(claimed_team) = claimed_team {
		let style = match claimed_slot.expect("Claimed team exists") {
			0..=2 => AllianceStyle::Red,
			3.. => AllianceStyle::Blue,
		};

		render_team(claimed_team, TeamStyle::Alliance(style))
	} else {
		String::new()
	};
	let out = out.replace("{{team}}", &claimed_team_elem);

	let claimed_team_data = if let Some(claimed_team) = claimed_team {
		claimed_team.to_string()
	} else {
		"\"\"".into()
	};
	let out = out.replace("{{data-team}}", &claimed_team_data);

	// Check if match stats have been reported for this match and team to let the user know if they are done
	let is_done = if let Some(claimed_team) = claimed_team {
		match_stats.iter().any(|x| {
			x.match_number.as_ref().is_some_and(|x| x == &m.num) && x.team_number == claimed_team
		})
	} else {
		false
	};
	let is_done_icon = if is_done {
		"<img src=/assets/icons/check.svg />"
	} else {
		""
	};
	let out = out.replace("{{done-icon}}", is_done_icon);

	out
}

/// Render one of the little team bubbles to show claim status
fn render_claim_bubble(claim: Option<&str>, style: AllianceStyle) -> String {
	let class = if claim.is_some() {
		match style {
			AllianceStyle::Red => " r",
			AllianceStyle::Blue => " b",
		}
	} else {
		""
	};
	format!("<div class=\"claim{class}\"></div>")
}

/// Style for one of the team bubbles or teams in a match
enum AllianceStyle {
	Red,
	Blue,
}

/// Style for one of the teams in a match
enum TeamStyle {
	Alliance(AllianceStyle),
	None,
}

fn render_team(team: TeamNumber, style: TeamStyle) -> String {
	let style = match style {
		TeamStyle::Alliance(AllianceStyle::Blue) => "style=\"border-color:var(--wbblueacc)\"",
		TeamStyle::Alliance(AllianceStyle::Red) => "style=\"border-color:var(--wbred)\"",
		TeamStyle::None => "",
	};
	format!("<a class=\"cont round nolink team\" href=\"/scouting/team/{team}\" {style}>{team}</a>")
}
