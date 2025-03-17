use std::{collections::HashMap, ops::Deref};

use rocket::{
	http::Status,
	response::{content::RawHtml, Redirect},
};
use tracing::{error, span, Level};

use crate::{
	api::statbotics::StatboticsClient,
	db::Database,
	routes::OptionalSessionID,
	scouting::{
		stats::CombinedTeamStats,
		status::{RobotStatus, StatusUpdate},
		DriveTrainType, Team, TeamInfo, TeamNumber,
	},
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

	let lock = state.db.read().await;

	// Collect teams and team info into two separate maps
	let mut db_teams_red = HashMap::with_capacity(3);
	let mut db_teams_blue = HashMap::with_capacity(3);
	let mut team_info_red = HashMap::with_capacity(3);
	let mut team_info_blue = HashMap::with_capacity(3);
	for (i, team) in teams.iter().enumerate() {
		if let Some(team) = team {
			if let Ok(Some(valid_team)) = lock.get_team(*team).await.map_err(|e| {
				error!("Failed to get team from database: {e}");
				e
			}) {
				if i < 3 {
					db_teams_red.insert(*team, valid_team);
				} else {
					db_teams_blue.insert(*team, valid_team);
				}
			}

			if let Ok(Some(valid_info)) = lock.get_team_info(*team).await.map_err(|e| {
				error!("Failed to get team info from database: {e}");
				e
			}) {
				if i < 3 {
					team_info_red.insert(*team, valid_info);
				} else {
					team_info_blue.insert(*team, valid_info);
				}
			}
		}
	}

	let statuses = lock.get_all_status().await.map_err(|e| {
		error!("Failed to get statuses from database: {e}");
		Status::InternalServerError
	})?;

	// Don't include the breakdowns if no teams are provided
	let page = if teams.iter().any(|x| x.is_some()) {
		let red_alliance = if teams.len() > 3 {
			&teams[0..3]
		} else {
			&teams[0..]
		};
		let page = page.replace(
			"{{red-breakdown}}",
			&render_alliance_breakdown(
				AllianceColor::Red,
				red_alliance,
				team_stats.deref(),
				&db_teams_red,
				&team_info_red,
				&state.statbotics_client,
				&statuses,
			)
			.await,
		);
		let blue_alliance = if teams.len() > 3 { &teams[3..] } else { &[] };
		let page = page.replace(
			"{{blue-breakdown}}",
			&render_alliance_breakdown(
				AllianceColor::Blue,
				blue_alliance,
				team_stats.deref(),
				&db_teams_blue,
				&team_info_blue,
				&state.statbotics_client,
				&statuses,
			)
			.await,
		);

		page
	} else {
		let page = page.replace("{{red-breakdown}}", "");
		let page = page.replace("{{blue-breakdown}}", "");

		page
	};

	let page = create_page("Matchup", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

/// Render one alliance breakdown for a matchup
async fn render_alliance_breakdown(
	alliance: AllianceColor,
	teams: &[Option<TeamNumber>],
	team_stats: &HashMap<TeamNumber, CombinedTeamStats>,
	db_teams: &HashMap<TeamNumber, Team>,
	team_info: &HashMap<TeamNumber, TeamInfo>,
	statbotics_client: &StatboticsClient,
	statuses: &[StatusUpdate],
) -> String {
	let default_stats = CombinedTeamStats::default();
	let mut all_stats = Vec::new();

	let mut point_total = 0.0;

	for team in teams {
		if let Some(team) = team {
			let stats = team_stats.get(&team).unwrap_or(&default_stats);
			point_total += get_expected_points(
				stats,
				statbotics_client
					.get_epa(*team)
					.await
					.unwrap_or(stats.current_competition.apa),
			);
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

	let out = out.replace("{{expected-points}}", &format!("{point_total:.1}"));

	// Why can't floats just be ord
	let mut max = 0.0;
	let mut max_team = None;
	for (team, stats) in &all_stats {
		if stats.current_competition.apa > max {
			max = stats.current_competition.apa;
			max_team = Some(team);
		}
	}
	let mvp = max_team
		.map(|x| x.to_string())
		.unwrap_or(String::from("None"));

	let out = out.replace("{{mvp}}", &mvp);

	// Create tips
	let mut tips_string = String::new();

	// These tips being added should be ordered so that the most important ones are first and at the top in the breakdown

	for team in db_teams.keys() {
		let status = RobotStatus::get_from_updates(statuses.iter().filter(|x| x.team == *team));
		if status == RobotStatus::Questionable {
			tips_string.push_str(&Tip::Questionable(*team).render());
		} else if status == RobotStatus::Broken {
			tips_string.push_str(&Tip::Broken(*team).render());
		}
	}

	for (team, stats) in &all_stats {
		if stats.current_competition.defense_average > 0.25 {
			tips_string.push_str(&Tip::StrongDefense(*team).render());
		}
	}

	if let Some(geezer) = db_teams.values().find(|x| x.rookie_year <= 2008) {
		tips_string.push_str(&Tip::VeteranTeam(geezer.number).render());
	}

	if let Some(mecanum) = team_info
		.iter()
		.find(|x| x.1.drivetrain_type == Some(DriveTrainType::Mecanum))
		.map(|x| x.0)
	{
		tips_string.push_str(&Tip::MecanumBot(*mecanum).render());
	}

	if let Some(tank) = team_info
		.iter()
		.find(|x| x.1.drivetrain_type == Some(DriveTrainType::Tank))
		.map(|x| x.0)
	{
		tips_string.push_str(&Tip::TankBot(*tank).render());
	}

	if let Some(zoomer) = team_info
		.iter()
		.find(|x| x.1.max_speed.is_some_and(|x| x >= 16.0))
		.map(|x| x.0)
	{
		tips_string.push_str(&Tip::HighSpeed(*zoomer).render());
	}

	// If it isn't filled out, assume that they can amp
	if team_info.values().all(|x| !x.can_processor.unwrap_or(true)) {
		tips_string.push_str(&Tip::CantProcess.render());
	}

	let out = out.replace("{{tips}}", &tips_string);

	let coral_breakdown_1 = render_team_coral_breakdown(
		teams[0].unwrap_or_default(),
		team_stats
			.get(&teams[0].unwrap_or_default())
			.unwrap_or(&default_stats),
		alliance,
	);
	let coral_breakdown_2 = render_team_coral_breakdown(
		teams[1].unwrap_or_default(),
		team_stats
			.get(&teams[1].unwrap_or_default())
			.unwrap_or(&default_stats),
		alliance,
	);
	let coral_breakdown_3 = render_team_coral_breakdown(
		teams[2].unwrap_or_default(),
		team_stats
			.get(&teams[2].unwrap_or_default())
			.unwrap_or(&default_stats),
		alliance,
	);
	let out = out.replace("{{coral-breakdown-1}}", &coral_breakdown_1);
	let out = out.replace("{{coral-breakdown-2}}", &coral_breakdown_2);
	let out = out.replace("{{coral-breakdown-3}}", &coral_breakdown_3);

	out
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum AllianceColor {
	Red,
	Blue,
}

enum Tip {
	Questionable(TeamNumber),
	Broken(TeamNumber),
	VeteranTeam(TeamNumber),
	MecanumBot(TeamNumber),
	TankBot(TeamNumber),
	HighSpeed(TeamNumber),
	StrongDefense(TeamNumber),
	CantProcess,
}

impl Tip {
	/// Render a single tip for an alliance breakdown
	fn render(self) -> String {
		let title = match &self {
			Self::Questionable(team) => format!("Questionable:<br />{team}"),
			Self::Broken(team) => format!("Broken:<br />{team}"),
			Self::VeteranTeam(team) => format!("Veteran Team:<br />{team}"),
			Self::MecanumBot(team) => format!("Mechanum Bot:<br />{team}"),
			Self::TankBot(team) => format!("Tank Bot:<br />{team}"),
			Self::HighSpeed(team) => format!("High Speed:<br />{team}"),
			Self::StrongDefense(team) => format!("Strong Defense:<br />{team}"),
			Self::CantProcess => "Can't Process".into(),
		};

		let urgent_class = match &self {
			Self::Questionable(..) | Self::Broken(..) => " urgent",
			_ => "",
		};

		format!("<div class=\"cont round tip {urgent_class}\">{title}</div>")
	}
}

/// Get expected points from a team as a combination of EPA and APA
fn get_expected_points(stats: &CombinedTeamStats, epa: f32) -> f32 {
	let epa_weight = 3.0;
	let all_time_weight = 5.0;
	let current_comp_weight = stats.current_competition.matches as f32;
	let total = epa_weight + all_time_weight + current_comp_weight;

	(epa * epa_weight
		+ stats.all_time.apa * all_time_weight
		+ stats.current_competition.apa * current_comp_weight)
		/ total
}

fn render_team_coral_breakdown(
	team: TeamNumber,
	stats: &CombinedTeamStats,
	alliance: AllianceColor,
) -> String {
	let out = include_str!("../components/scouting/team_coral_breakdown.min.html");
	let out = out.replace("{{team}}", &team.to_string());

	let out = out.replace(
		"\"{{bg-color}}\"",
		match alliance {
			AllianceColor::Red => "var(--wbredbg)",
			AllianceColor::Blue => "var(--wbbluebg)",
		},
	);

	let out = out.replace("{{l1}}", &format!("{:.2}", stats.all_time.l1_value));
	let out = out.replace("{{l2}}", &format!("{:.2}", stats.all_time.l2_value));
	let out = out.replace("{{l3}}", &format!("{:.2}", stats.all_time.l3_value));
	let out = out.replace("{{l4}}", &format!("{:.2}", stats.all_time.l4_value));

	out
}
