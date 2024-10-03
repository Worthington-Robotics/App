use itertools::Itertools;
use rocket::{
	http::Status,
	response::{content::RawHtml, Redirect},
};
use tracing::{error, span, Level};

use crate::{
	db::Database,
	member::Member,
	routes::{OptionalSessionID, SessionID},
	scouting::{
		assignment::{assign_scouts, ScoutingAssignment},
		Competition, TeamNumber,
	},
	State,
};

use super::{create_page, PageOrRedirect, Scope};

#[rocket::get("/scouting/assignments")]
pub async fn assignments(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Scouting assignments");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.verify_elevated(state).await.is_err() {
		return Ok(redirect);
	}

	let page = include_str!("../pages/scouting/assignments.min.html");

	let current_comp = Competition::Pittsburgh;

	let lock = state.db.lock().await;
	let teams = lock.get_teams().await.map_err(|e| {
		error!("Failed to get teams from database: {e}");
		Status::InternalServerError
	})?;
	let teams = teams.filter(|x| x.competitions.contains(&current_comp));
	let teams = teams.sorted_by_key(|x| x.number);

	let assignments = lock.get_all_assignments().await.map_err(|e| {
		error!("Failed to get all scouting assignments from database: {e}");
		Status::InternalServerError
	})?;
	let assignments: Vec<_> = assignments.collect();

	let mut available_teams_str = String::new();
	for team in teams {
		if assignments.iter().any(|x| x.teams.contains(&team.number)) {
			continue;
		}

		available_teams_str.push_str(&render_team(team.number, None));
	}
	let page = page.replace("{{available-teams}}", &available_teams_str);

	let members = lock.get_members().await.map_err(|e| {
		error!("Failed to get all members from database: {e}");
		Status::InternalServerError
	})?;

	let mut members_str = String::new();
	for member in members.sorted_by_key(|x| x.name.clone()) {
		let assignment = assignments
			.iter()
			.find(|x| x.member == member.id)
			.cloned()
			.unwrap_or(ScoutingAssignment {
				member: member.id.clone(),
				..Default::default()
			});

		members_str.push_str(&render_member(member, assignment));
	}
	let page = page.replace("{{members}}", &members_str);

	let page = create_page("Scouting Assignments", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

/// Renders an assignable team component
fn render_team(team: TeamNumber, member: Option<&str>) -> String {
	let assigned_class = if member.is_some() { "" } else { "unassigned" };
	let member = member.unwrap_or_default();
	format!("<div class=\"cont round team {assigned_class}\" draggable=true id=team-{team} data-member={member}>{team}</div>")
}

/// Renders a member section component
fn render_member(member: Member, assignment: ScoutingAssignment) -> String {
	let mut teams_str = String::new();
	for team in assignment.teams {
		teams_str.push_str(&render_team(team, Some(&member.id)));
	}
	format!(
		"<div class=\"round member\"><div class=\"cont member-name\">{}</div><div class=\"round member-teams\" data-id={}>{teams_str}</div></div>",
		member.name,
		member.id
	)
}

#[rocket::post("/api/assign_team/<member>/<team>")]
pub async fn assign_team(
	state: &State,
	session_id: SessionID<'_>,
	member: &str,
	team: TeamNumber,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Assigning team");
	let _enter = span.enter();

	session_id.verify_elevated(state).await?;

	let mut lock = state.db.lock().await;
	if !lock.member_exists(member).await.map_err(|e| {
		error!("Failed to check if member exists: {e}");
		Status::InternalServerError
	})? {
		return Err(Status::NotFound);
	}

	let mut assignment = lock
		.get_assignment(member)
		.await
		.map_err(|e| {
			error!("Failed to get assignment from database: {e}");
			Status::InternalServerError
		})?
		.unwrap_or(ScoutingAssignment {
			member: member.to_string(),
			..Default::default()
		});

	if assignment.teams.contains(&team) {
		return Ok(());
	}
	assignment.teams.push(team);

	if let Err(e) = lock.create_assignment(assignment).await {
		error!("Failed to update assignment for member {member} in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

#[rocket::post("/api/unassign_team/<member>/<team>")]
pub async fn unassign_team(
	state: &State,
	session_id: SessionID<'_>,
	member: &str,
	team: TeamNumber,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Unassigning team");
	let _enter = span.enter();

	session_id.verify_elevated(state).await?;

	let mut lock = state.db.lock().await;
	if !lock.member_exists(member).await.map_err(|e| {
		error!("Failed to check if member exists: {e}");
		Status::InternalServerError
	})? {
		return Err(Status::NotFound);
	}
	let mut assignment = lock
		.get_assignment(member)
		.await
		.map_err(|e| {
			error!("Failed to get assignment from database: {e}");
			Status::InternalServerError
		})?
		.unwrap_or(ScoutingAssignment {
			member: member.to_string(),
			..Default::default()
		});

	let Some(index) = assignment.teams.iter().position(|x| x == &team) else {
		return Ok(());
	};
	assignment.teams.remove(index);

	if let Err(e) = lock.create_assignment(assignment).await {
		error!("Failed to update assignment for member {member} in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

#[rocket::post("/api/random_assign_teams")]
pub async fn random_assign(state: &State, session_id: SessionID<'_>) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Randomly assigning teams");
	let _enter = span.enter();

	session_id.verify_elevated(state).await?;

	let mut lock = state.db.lock().await;

	let current_comp = Competition::Pittsburgh;

	let teams = lock.get_teams().await.map_err(|e| {
		error!("Failed to get teams from database: {e}");
		Status::InternalServerError
	})?;
	let teams = teams.filter(|x| x.competitions.contains(&current_comp));
	let teams = teams.map(|x| x.number);
	let teams = teams.sorted();
	let teams: Vec<_> = teams.collect();

	let members = lock.get_members().await.map_err(|e| {
		error!("Failed to get all members from database: {e}");
		Status::InternalServerError
	})?;
	let members: Vec<_> = members.map(|x| x.id).collect();

	let assignments = assign_scouts(&teams, &members);
	for assignment in assignments {
		if let Err(e) = lock.create_assignment(assignment).await {
			error!("Failed to update assignment in database: {e}");
			return Err(Status::InternalServerError);
		}
	}

	Ok(())
}
