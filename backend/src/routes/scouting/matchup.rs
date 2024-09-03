use std::{collections::HashMap, ops::Deref};

use rocket::{
	http::Status,
	response::{content::RawHtml, Redirect},
};
use tracing::{span, Level};

use crate::{
	routes::OptionalSessionID,
	scouting::{TeamNumber, TeamStats},
	State,
};

use super::{create_page, PageOrRedirect, Scope};

#[rocket::get("/scouting/matchup?<team>")]
pub async fn matchup(
	session_id: OptionalSessionID<'_>,
	state: &State,
	team: Vec<Option<TeamNumber>>,
) -> Result<PageOrRedirect, Status> {
	let teams = team;

	let span = span!(Level::DEBUG, "Team matchup");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.get_requesting_member(state).await.is_err() {
		return Ok(redirect);
	};

	let mut page = include_str!("../pages/scouting/matchup.min.html").to_string();

	// This method for filling out the teams won't replace all the teams if the list doesn't specify all 6,
	// but that's ok because they don't show up in number inputs and are parsed as none by Rocket
	for (i, team) in teams.iter().enumerate() {
		let value = if let Some(team) = &team {
			team.to_string()
		} else {
			String::new()
		};
		page = page.replace(&format!("{{{{team{}}}}}", i + 1), &value);
	}

	let team_stats = state.team_stats.read().await;

	let red_alliance = if teams.len() > 3 {
		&teams[0..3]
	} else {
		&teams[0..]
	};
	let page = page.replace(
		"{{red-breakdown}}",
		&render_alliance_breakdown(AllianceColor::Red, red_alliance, team_stats.deref()),
	);
	let blue_alliance = if teams.len() > 3 { &teams[3..] } else { &[] };
	let page = page.replace(
		"{{blue-breakdown}}",
		&render_alliance_breakdown(AllianceColor::Blue, blue_alliance, team_stats.deref()),
	);

	let page = create_page("Matchup", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

/// Render one alliance breakdown for a matchup
fn render_alliance_breakdown(
	alliance: AllianceColor,
	teams: &[Option<TeamNumber>],
	team_stats: &HashMap<TeamNumber, TeamStats>,
) -> String {
	let default_stats = TeamStats::default();
	let mut all_stats = Vec::new();

	let mut point_total = 0.0;

	for team in teams {
		if let Some(team) = team {
			let stats = team_stats.get(&team).unwrap_or(&default_stats);
			point_total += stats.apa;
			all_stats.push((*team, stats));
		}
	}

	let out = include_str!("../components/scouting/alliance_breakdown.min.html");

	let out = out.replace(
		"\"{{border-color}}\"",
		match alliance {
			AllianceColor::Red => "var(--wbred)",
			AllianceColor::Blue => "var(--wbblue)",
		},
	);

	let out = out.replace("{{expected-points}}", &point_total.to_string());

	// Why can't floats just be ord
	let mut max = 0.0;
	let mut max_team = None;
	for (team, stats) in &all_stats {
		if stats.apa > max {
			max = stats.apa;
			max_team = Some(team);
		}
	}
	let mvp = max_team
		.map(|x| x.to_string())
		.unwrap_or(String::from("None"));

	let out = out.replace("{{mvp}}", &mvp);

	out
}

#[derive(PartialEq, Eq)]
enum AllianceColor {
	Red,
	Blue,
}
