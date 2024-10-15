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
		assignment::{assign_scouts, MatchClaims, ScoutingAssignment},
		matches::{MatchNumber, MatchType},
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

	let assignments = lock.get_all_prescouting_assignments().await.map_err(|e| {
		error!("Failed to get all prescouting assignments from database: {e}");
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
		.get_prescouting_assignment(member)
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

	if let Err(e) = lock.create_prescouting_assignment(assignment).await {
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
		.get_prescouting_assignment(member)
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

	if let Err(e) = lock.create_prescouting_assignment(assignment).await {
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
		if let Err(e) = lock.create_prescouting_assignment(assignment).await {
			error!("Failed to update assignment in database: {e}");
			return Err(Status::InternalServerError);
		}
	}

	Ok(())
}

#[rocket::post("/api/claim_match/<match>/<slot>")]
pub async fn claim_match(
	state: &State,
	session_id: SessionID<'_>,
	r#match: u16,
	slot: u8,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Claiming match");
	let _enter = span.enter();

	let m = r#match;

	let requesting_member = session_id.get_requesting_member(state).await?;

	let mut lock = state.db.lock().await;

	let match_number = MatchNumber {
		num: m,
		ty: MatchType::Qualification,
	};

	let mut claims = lock
		.get_match_claims(&match_number)
		.await
		.map_err(|e| {
			error!("Failed to get match claims from database: {e}");
			Status::InternalServerError
		})?
		.unwrap_or(MatchClaims {
			m: match_number,
			..Default::default()
		});

	// Wow, maybe we should make an iterator
	if claims
		.red_1
		.as_ref()
		.is_some_and(|x| x == &requesting_member.id)
		|| claims
			.red_2
			.as_ref()
			.is_some_and(|x| x == &requesting_member.id)
		|| claims
			.red_3
			.as_ref()
			.is_some_and(|x| x == &requesting_member.id)
		|| claims
			.blue_1
			.as_ref()
			.is_some_and(|x| x == &requesting_member.id)
		|| claims
			.blue_2
			.as_ref()
			.is_some_and(|x| x == &requesting_member.id)
		|| claims
			.blue_3
			.as_ref()
			.is_some_and(|x| x == &requesting_member.id)
	{
		return Ok(());
	}

	let slot = match slot {
		0 => &mut claims.red_1,
		1 => &mut claims.red_2,
		2 => &mut claims.red_3,
		3 => &mut claims.blue_1,
		4 => &mut claims.blue_2,
		5 => &mut claims.blue_3,
		_ => return Err(Status::BadRequest),
	};

	*slot = Some(requesting_member.id);

	if let Err(e) = lock.create_match_claims(claims).await {
		error!("Failed to update claims in database: {e:#}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

#[rocket::post("/api/claim_best/<match>")]
pub async fn claim_best(
	state: &State,
	session_id: SessionID<'_>,
	r#match: u16,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Claiming best team from match");
	let _enter = span.enter();

	let m = r#match;

	let lock = state.db.lock().await;

	let match_number = MatchNumber {
		num: m,
		ty: MatchType::Qualification,
	};

	// TODO: Make this actually pick the best team

	let claims = lock
		.get_match_claims(&match_number)
		.await
		.map_err(|e| {
			error!("Failed to get match claims from database: {e}");
			Status::InternalServerError
		})?
		.unwrap_or(MatchClaims {
			m: match_number,
			..Default::default()
		});

	let slot = &[
		claims.red_1,
		claims.red_2,
		claims.red_3,
		claims.blue_1,
		claims.blue_2,
		claims.blue_3,
	]
	.iter()
	.position(|x| x.is_none());

	// Prevent deadlock since we will now call this other API method
	std::mem::drop(lock);

	if let Some(slot) = slot {
		claim_match(state, session_id, m, *slot as u8).await?;
	}

	Ok(())
}

#[rocket::post("/api/unclaim_match/<match>")]
pub async fn unclaim_match(
	state: &State,
	session_id: SessionID<'_>,
	r#match: u16,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Unclaiming match");
	let _enter = span.enter();

	let m = r#match;

	let requesting_member = session_id.get_requesting_member(state).await?;

	let mut lock = state.db.lock().await;

	let match_number = MatchNumber {
		num: m,
		ty: MatchType::Qualification,
	};

	let mut claims = lock
		.get_match_claims(&match_number)
		.await
		.map_err(|e| {
			error!("Failed to get match claims from database: {e}");
			Status::InternalServerError
		})?
		.unwrap_or(MatchClaims {
			m: match_number,
			..Default::default()
		});

	if claims
		.red_1
		.as_ref()
		.is_some_and(|x| x == &requesting_member.id)
	{
		claims.red_1 = None;
	} else if claims
		.red_2
		.as_ref()
		.is_some_and(|x| x == &requesting_member.id)
	{
		claims.red_2 = None;
	} else if claims
		.red_3
		.as_ref()
		.is_some_and(|x| x == &requesting_member.id)
	{
		claims.red_3 = None;
	} else if claims
		.blue_1
		.as_ref()
		.is_some_and(|x| x == &requesting_member.id)
	{
		claims.blue_1 = None;
	} else if claims
		.blue_2
		.as_ref()
		.is_some_and(|x| x == &requesting_member.id)
	{
		claims.blue_2 = None;
	} else if claims
		.blue_3
		.as_ref()
		.is_some_and(|x| x == &requesting_member.id)
	{
		claims.blue_3 = None;
	}

	if let Err(e) = lock.create_match_claims(claims).await {
		error!("Failed to update claims in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}
