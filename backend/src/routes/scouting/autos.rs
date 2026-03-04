use std::{collections::HashMap, io::Cursor, ops::Deref, sync::Arc};

use chrono::Utc;
use itertools::Itertools;
use rocket::{
	form::{Form, FromForm},
	http::Status,
	response::{
		content::{RawHtml, RawJson},
		Redirect,
	},
};
use svg::node::element::{path::Data as PathData, Path, Style, Text};
use tracing::{error, span, Level};

use crate::{
	db::Database,
	routes::{create_page, OptionalSessionID, PageOrRedirect, Scope, SessionID},
	scouting::autos::{get_auto_event_graphs, AutoEventGraphs},
	util::generate_id,
	AutoImageCacheEntry, State,
};
use crate::{
	routes::assets::{CacheFor, SvgDynamic},
	scouting::{
		autos::{Auto, AutoStats},
		TeamNumber,
	},
};

use super::stats::{render_stat_card_float, render_stat_card_pct, STAT_FUEL};

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
		auto_map.entry(auto.fuel).or_insert(Vec::new()).push(auto);
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
	let out = out.replace("{{fuel}}", &format!("{:.2}", stats.average_fuel));

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
		fuel: auto.fuel,
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
	fuel: u8,
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
	let page = page.replace("{{fuel}}", &auto.fuel.to_string());

	// Create stats
	let default_stats = AutoStats::default();
	let lock2 = state.auto_stats.read().await;
	let auto_stats = lock2.get(id).unwrap_or(&default_stats);

	let page = page.replace(
		"{{point-value}}",
		&render_stat_card_float("Avg Points", "", auto_stats.point_value, true, ""),
	);
	let page = page.replace(
		"{{average-fuel}}",
		&render_stat_card_float(
			&format!("{STAT_FUEL} Avg"),
			"",
			auto_stats.average_fuel,
			true,
			"",
		),
	);
	let page = page.replace(
		"{{fuel-accuracy}}",
		&render_stat_card_pct(
			&format!("{STAT_FUEL} Acc"),
			"",
			auto_stats.fuel_accuracy,
			false,
			"",
		),
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

#[rocket::get("/scouting/auto/<team>/image.svg")]
pub async fn auto_image(
	state: &State,
	session_id: SessionID<'_>,
	team: TeamNumber,
) -> Result<CacheFor<SvgDynamic>, Status> {
	let span = span!(Level::DEBUG, "Getting auto image");
	let _enter = span.enter();

	session_id.get_requesting_member(state).await?;

	let mut images_lock = state.auto_images.lock().await;
	let db_lock = state.db.read().await;

	let mut image = images_lock.get(&team);
	if let Some(image2) = image {
		if (Utc::now() - image2.entry_time).num_seconds() > 100 {
			image = None;
		}
	}

	let image = if let Some(image) = image {
		image.clone()
	} else {
		let matches = db_lock.get_all_match_stats().await.map_err(|e| {
			error!("Failed to get match stats from database: {e}");
			Status::InternalServerError
		})?;
		let matches: Vec<_> = matches.filter(|x| x.team_number == team).collect();

		let graphs = get_auto_event_graphs(&matches);

		let image = render_auto_image(&graphs);

		let entry = Arc::new(AutoImageCacheEntry {
			image,
			entry_time: Utc::now(),
		});
		images_lock.insert(team, entry.clone());
		entry
	};

	Ok(CacheFor(SvgDynamic(image.image.clone()), 200))
}

/// Renders an auto as an SVG image
pub fn render_auto_image(graphs: &AutoEventGraphs) -> Vec<u8> {
	let image_height = 160.0;
	let image_width = 240.0;

	let graph_start_x = 30.0;
	let graph_end_x = image_width - 5.0;
	let all_graphs_height = 120.0;
	let graph_separation = 2.0;

	let mut document = svg::Document::new().set("viewBox", (0, 0, image_width, image_height));

	// Draw the graphs
	for (i, (graph_title, color, graph)) in
		[("Fuel", "#f5e042", &graphs.shots)].into_iter().enumerate()
	{
		let dx = (graph_end_x - graph_start_x) / graph.len() as f32;
		let dy = all_graphs_height / 6.0;

		// Start from the middle and move up and down
		let y_start = dy * i as f32 + graph_separation / 2.0;
		let y_end = y_start + dy * 0.5 - graph_separation / 2.0;
		let y_range = y_end - y_start;

		let mut point_data = PathData::new();
		let first = graph[0];
		point_data = point_data.move_to((graph_start_x, y_end - first * y_range));

		for (j, value) in graph.iter().enumerate() {
			let x = graph_start_x + j as f32 * dx;
			let y = y_end - *value * y_range;
			point_data = point_data.line_to((x, y));
		}

		let path = Path::new()
			.set("fill", "none")
			.set("stroke", color)
			.set("stroke-width", 2.75)
			.set("stroke-linejoin", "round")
			.set("d", point_data);

		document = document.add(path);

		let text = Text::new(graph_title)
			.set("x", graph_start_x * 0.05)
			.set("y", y_end + 0.5 * y_range)
			.set("class", "graph-title");
		document = document.add(text);
	}

	// Draw time intervals on the bottom axis
	for i in 0..7 {
		let x = graph_start_x + (graph_end_x - graph_start_x) / 6.5 * i as f32;
		let y = all_graphs_height + 10.0;

		let text = format!("{:.0}", 15.0 / 6.0 * i as f32);

		let text = Text::new(text).set("x", x).set("y", y).set("class", "time");
		document = document.add(text);
	}

	document = document.add(Style::new(
		r#"
		@font-face {
			font-family: 'Rubik2';
			font-style: normal;
			font-weight: 300 900;
			src: url(https://fonts.gstatic.com/s/rubik/v28/iJWKBXyIfDnIV7nBrXyw1W3fxIk.woff2) format('woff2');
			unicode-range: U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308, U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD;
		}

		.graph-title {
			fill: #f5f5f5;
			font: 15px "Rubik2", sans-serif;
		}

		.time {
			fill: #f5f5f5;
			font: 10px "Rubik2", sans-serif;
		}
	"#,
	));

	let mut out = Cursor::new(Vec::new());
	svg::write(&mut out, &document).expect("Should have no errors writing to a string");

	out.into_inner()
}
