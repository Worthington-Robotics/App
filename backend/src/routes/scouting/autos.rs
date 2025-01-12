use std::{collections::HashMap, ops::Deref};

use itertools::Itertools;
use rocket::{
	form::{Form, FromForm},
	http::Status,
	response::{
		content::{RawHtml, RawJson},
		Redirect,
	},
};
use tracing::{error, span, Level};

use crate::scouting::{
	autos::{Auto, AutoStats},
	TeamNumber,
};
use crate::{
	db::Database,
	routes::{create_page, OptionalSessionID, PageOrRedirect, Scope, SessionID},
	util::generate_id,
	State,
};

use super::stats::{
	render_stat_card_float, render_stat_card_optional_bool, render_stat_card_pct, STAT_ALGAE,
	STAT_CORAL,
};

#[rocket::get("/scouting/team/<team>/autos")]
pub async fn autos_page(
	session_id: OptionalSessionID<'_>,
	state: &State,
	team: TeamNumber,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Team autos page");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.get_requesting_member(state).await.is_err() {
		return Ok(redirect);
	};

	let page = include_str!("../pages/scouting/team/autos.min.html");
	let page = page.replace("{{team-number}}", &team.to_string());

	let lock = state.db.read().await;
	let autos = lock.get_autos(team).await.map_err(|e| {
		error!("Failed to get autos from database: {e}");
		Status::InternalServerError
	})?;

	// Group autos by number of game pieces
	let mut auto_map = HashMap::with_capacity(autos.size_hint().0);

	for auto in autos {
		auto_map
			.entry(auto.coral + auto.algae)
			.or_insert(Vec::new())
			.push(auto);
	}

	let auto_stats = state.auto_stats.read().await;

	// Create the sections, with autos using the most pieces at the top
	let mut sections_string = String::new();
	for (piece_count, group_autos) in auto_map
		.into_iter()
		.sorted_by_key(|x| std::cmp::Reverse(x.0))
	{
		let section = render_auto_section(piece_count, &group_autos, auto_stats.deref());
		sections_string.push_str(&section);
	}
	let page = page.replace("{{autos}}", &sections_string);

	let page = page.replace(
		"{{add-button}}",
		include_str!("../components/ui/new.min.html"),
	);

	let page = create_page("Team Autos", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

/// Create a section of autos with the same number of game pieces
fn render_auto_section(
	piece_count: u8,
	autos: &[Auto],
	auto_stats: &HashMap<String, AutoStats>,
) -> String {
	let out = include_str!("../components/scouting/autos/section.min.html");

	let piece_count_word = if piece_count == 1 { "Piece" } else { "Pieces" };
	let out = out.replace(
		"{{piece-count}}",
		&format!("{piece_count} {piece_count_word}"),
	);

	let mut autos_string = String::new();
	for auto in autos {
		let stats = auto_stats.get(&auto.id).cloned().unwrap_or_default();
		let auto = render_auto(auto, &stats);
		autos_string.push_str(&auto);
	}

	let out = out.replace("{{autos}}", &autos_string);

	out
}

/// Render a single auto
fn render_auto(auto: &Auto, stats: &AutoStats) -> String {
	let out = include_str!("../components/scouting/autos/auto.min.html");
	let out = out.replace("{{id}}", &auto.id);
	let out = out.replace("{{name}}", &auto.name);
	let out = out.replace("{{average-score}}", &format!("{:.2}", stats.point_value));
	let out = out.replace("{{coral}}", &format!("{:.2}", stats.average_coral));
	let out = out.replace("{{algae}}", &format!("{:.2}", stats.average_algae));

	out
}

#[rocket::get("/scouting/create_auto/<team>")]
pub async fn create_auto_page(
	session_id: OptionalSessionID<'_>,
	state: &State,
	team: TeamNumber,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Create auto page");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.get_requesting_member(state).await.is_err() {
		return Ok(redirect);
	};

	let page = include_str!("../pages/scouting/team/create_auto.min.html");
	let page = page.replace("{{team-number}}", &team.to_string());

	let page = create_page("Create Auto", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

#[rocket::post("/api/create_auto", data = "<auto>")]
pub async fn create_auto(
	state: &State,
	session_id: SessionID<'_>,
	auto: Form<AutoForm>,
) -> Result<(), Status> {
	let auto = auto.into_inner();

	let span = span!(Level::DEBUG, "Creating auto");
	let _enter = span.enter();

	session_id.get_requesting_member(state).await?;

	let id = generate_id();
	let auto = Auto {
		id,
		name: auto.name,
		team: auto.team,
		coral: auto.coral,
		algae: auto.algae,
		agitates: auto.agitates,
		starting_position: auto.starting_position,
	};

	let mut lock = state.db.write().await;

	if let Err(e) = lock.create_auto(auto).await {
		error!("Failed to create auto in database: {e:#}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

#[derive(FromForm)]
pub struct AutoForm {
	name: String,
	team: TeamNumber,
	coral: u8,
	algae: u8,
	agitates: bool,
	starting_position: f32,
}

#[rocket::get("/scouting/auto/<id>")]
pub async fn auto_details(
	id: &str,
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Auto details page");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.get_requesting_member(state).await.is_err() {
		return Ok(redirect);
	};

	let lock = state.db.read().await;
	let auto = lock
		.get_auto(id)
		.await
		.map_err(|e| {
			error!("Failed to get auto from database: {e}");
			Status::InternalServerError
		})?
		.ok_or_else(|| {
			error!("Auto does not exist: {}", id);
			Status::NotFound
		})?;

	let page = include_str!("../pages/scouting/team/auto_details.min.html");
	let page = page.replace("{{id}}", &auto.id);
	let page = page.replace("{{name}}", &auto.name);
	let page = page.replace("{{coral}}", &auto.coral.to_string());
	let page = page.replace("{{algae}}", &auto.algae.to_string());

	// Create stats
	let default_stats = AutoStats::default();
	let lock2 = state.auto_stats.read().await;
	let auto_stats = lock2.get(id).unwrap_or(&default_stats);

	let page = page.replace(
		"{{point-value}}",
		&render_stat_card_float("Avg Points", "", auto_stats.point_value, true, ""),
	);
	let page = page.replace(
		"{{average-coral}}",
		&render_stat_card_float(
			&format!("{STAT_CORAL} Avg"),
			"",
			auto_stats.average_coral,
			true,
			"",
		),
	);
	let page = page.replace(
		"{{average-algae}}",
		&render_stat_card_float(
			&format!("{STAT_ALGAE} Avg"),
			"",
			auto_stats.average_algae,
			true,
			"",
		),
	);
	let page = page.replace(
		"{{coral-accuracy}}",
		&render_stat_card_pct(
			&format!("{STAT_CORAL} Acc"),
			"",
			auto_stats.coral_accuracy,
			false,
			"",
		),
	);
	let page = page.replace(
		"{{algae-accuracy}}",
		&render_stat_card_pct(
			&format!("{STAT_ALGAE} Avg"),
			"",
			auto_stats.algae_accuracy,
			false,
			"",
		),
	);
	let page = page.replace(
		"{{agitates}}",
		&render_stat_card_optional_bool("Agitates?", "", Some(auto.agitates), false, ""),
	);
	let page = page.replace(
		"{{usage-rate}}",
		&render_stat_card_pct("Usage", "", auto_stats.usage_rate, true, ""),
	);

	let page = create_page("Auto Details", &page, Some(Scope::Scouting));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

#[rocket::post("/api/rename_auto/<auto>?<name>")]
pub async fn rename_auto(
	state: &State,
	session_id: SessionID<'_>,
	auto: &str,
	name: String,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Renaming auto");
	let _enter = span.enter();

	session_id.get_requesting_member(state).await?;

	let mut lock = state.db.write().await;

	let Some(mut auto) = lock.get_auto(auto).await.map_err(|e| {
		error!("Failed to get auto from database: {e}");
		Status::InternalServerError
	})?
	else {
		error!("Auto {auto} does not exist");
		return Err(Status::NotFound);
	};

	auto.name = name;

	if let Err(e) = lock.create_auto(auto).await {
		error!("Failed to create auto with updated name in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

/// Get all the autos for a team. Used for filling out auto options for a match report
#[rocket::get("/api/get_autos/<team>")]
pub async fn get_autos(
	state: &State,
	session_id: SessionID<'_>,
	team: TeamNumber,
) -> Result<RawJson<String>, Status> {
	let span = span!(Level::DEBUG, "Getting autos");
	let _enter = span.enter();

	session_id.get_requesting_member(state).await?;

	let lock = state.db.read().await;

	let autos = lock.get_autos(team).await.map_err(|e| {
		error!("Failed to get autos from database: {e}");
		Status::InternalServerError
	})?;

	let autos: Vec<_> = autos.collect();

	let autos = serde_json::to_string(&autos).map_err(|e| {
		error!("Failed to serialize autos: {e}");
		Status::InternalServerError
	})?;

	Ok(RawJson(autos))
}
