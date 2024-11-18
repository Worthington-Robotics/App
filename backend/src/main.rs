use std::{
	collections::{HashMap, HashSet},
	sync::Arc,
};

use api::{first::FirstClient, statbotics::StatboticsClient};
use argon2::Argon2;
use attendance::AttendanceFairing;
use auth::SessionManager;
use chrono::DateTime;
#[cfg(feature = "cachedb")]
use db::cached::SyncCache;
use db::{Database, DatabaseImpl};
use dotenv::dotenv;
use member::Member;
use rocket::{
	catchers, routes,
	tokio::{
		join,
		sync::{Mutex, RwLock},
	},
};
use rocket_async_compression::CachedCompression;
use routes::{scouting::populate_teams, Ratelimit};
use scouting::{autos::AutoStats, stats::CombinedTeamStats, stats::UpdateStats, TeamNumber};

mod announcements;
mod api;
mod attendance;
mod auth;
mod db;
mod events;
mod forms;
mod member;
mod notifications;
mod routes;
mod scouting;
mod tasks;
mod util;

#[rocket::launch]
async fn rocket() -> _ {
	println!("Starting server...");
	let subscriber = tracing_subscriber::FmtSubscriber::new();
	tracing::subscriber::set_global_default(subscriber)
		.expect("Failed to set global tracing subscriber");

	let _ = dotenv();

	let mut session_manager = SessionManager::new();

	let session_id = session_manager.create("admin");
	println!("Session ID: {session_id}");

	let db_task = setup_db();

	// Load password hash
	let params = argon2::Params::new(15000, 2, 1, None).expect("Failed to build Argon2 parameters");

	let password_hash = Some(Argon2::new(
		argon2::Algorithm::Argon2id,
		argon2::Version::V0x13,
		params,
	));

	let req_client = reqwest::Client::new();

	let statbotics_task = async {
		let statbotics_client = StatboticsClient::new(&req_client);
		if std::env::var("POPULATE_EPA").is_ok_and(|x| x == "1") {
			statbotics_client
				.get_stats()
				.await
				.expect("Failed to get Statbotics stats");
		}

		statbotics_client
	};

	let (mut db, statbotics_client) = join!(db_task, statbotics_task);

	// Populate teams
	let first_client = FirstClient::new(&req_client);
	// This takes a while, so only do it if we need to
	if std::env::var("POPULATE_TEAMS").is_ok_and(|x| x == "1") {
		populate_teams(&mut db, &first_client)
			.await
			.expect("Failed to populate teams");
	}

	let team_stats = Arc::new(RwLock::new(HashMap::new()));
	let auto_stats = Arc::new(RwLock::new(HashMap::new()));

	let state = AppState {
		db: Arc::new(RwLock::new(db)),
		session_manager: Mutex::new(session_manager),
		password_hash,
		req_client,
		first_client,
		statbotics_client,
		team_stats: team_stats.clone(),
		auto_stats: auto_stats.clone(),
		auto_images: Arc::new(Mutex::new(HashMap::new())),
	};

	let db_clone = state.db.clone();
	#[cfg(feature = "cachedb")]
	let db_clone2 = state.db.clone();
	let db_clone3 = state.db.clone();

	let out = rocket::build()
		.manage(state)
		.mount(
			"/",
			routes![
				routes::index,
				routes::admin,
				routes::members::get_member,
				routes::members::create_member,
				routes::members::member_list,
				routes::members::create_member_page,
				routes::members::member_details,
				routes::members::delete_member,
				routes::members::update_member_form,
				routes::assets::favicon,
				routes::assets::main_css,
				routes::assets::static_css,
				routes::assets::sortable_js,
				routes::assets::error_js,
				routes::assets::prompt_js,
				routes::assets::logo,
				routes::assets::rockwell,
				routes::assets::icon_home,
				routes::assets::icon_clock,
				routes::assets::icon_plus,
				routes::assets::icon_mail,
				routes::assets::icon_edit,
				routes::assets::icon_delete,
				routes::assets::icon_check,
				routes::assets::icon_box,
				routes::assets::icon_eye,
				routes::assets::icon_star,
				routes::assets::icon_star_outline,
				routes::assets::icon_user,
				routes::assets::icon_calendar,
				routes::assets::icon_location,
				routes::login::login,
				routes::login::authenticate,
				routes::login::logout,
				routes::calendar::calendar,
				routes::calendar::create_event,
				routes::calendar::create_event_api,
				routes::calendar::event_details,
				routes::calendar::delete_event,
				routes::calendar::rsvp_event,
				routes::inbox::inbox,
				routes::inbox::create_announcement_api,
				routes::inbox::create_announcement_page,
				routes::inbox::announcement_details,
				routes::inbox::delete_announcement,
				routes::attendance::attend,
				routes::attendance::unattend,
				routes::tasks::create_checklist,
				routes::tasks::create_task,
				routes::tasks::update_task,
				routes::tasks::delete_checklist,
				routes::tasks::delete_task,
				routes::tasks::checklists,
				routes::tasks::create_checklist_page,
				routes::tasks::checklist_page,
				routes::settings::settings,
				routes::scouting::index,
				routes::scouting::admin,
				routes::scouting::download_data,
				routes::scouting::update_settings,
				routes::scouting::teams::teams,
				routes::scouting::teams::team_details,
				routes::scouting::teams::info::create_team_info,
				routes::scouting::teams::info::team_info_page,
				routes::scouting::teams::update_team_competition,
				routes::scouting::teams::update_team_following,
				routes::scouting::teams::get_historical_stat,
				routes::scouting::matches::create_match_stats,
				routes::scouting::matches::match_report_main,
				routes::scouting::matches::match_report_raw,
				routes::scouting::matches::match_schedule,
				routes::scouting::matches::import_match_schedule,
				routes::scouting::matches::clear_match_schedule,
				routes::scouting::matches::upload_match_schedule,
				routes::scouting::matchup::matchup,
				routes::scouting::autos::autos_page,
				routes::scouting::autos::create_auto,
				routes::scouting::autos::rename_auto,
				routes::scouting::autos::create_auto_page,
				routes::scouting::autos::get_autos,
				routes::scouting::autos::auto_details,
				routes::scouting::autos::auto_image,
				routes::scouting::status::team_status_page,
				routes::scouting::status::update_status,
				routes::scouting::assignments::assignments,
				routes::scouting::assignments::assign_team,
				routes::scouting::assignments::unassign_team,
				routes::scouting::assignments::random_assign,
				routes::scouting::assignments::claim_match,
				routes::scouting::assignments::claim_best,
				routes::scouting::assignments::unclaim_match,
				routes::scouting::my_scouting::my_scouting,
			],
		)
		.mount(
			"/cal",
			routes![
				routes::calendar::cal_call_get,
				routes::calendar::cal_call_post,
				routes::calendar::cal_call_propfind,
				routes::calendar::cal_call_report,
			],
		)
		.mount("/", routes![routes::calendar::cal_call_well_known,])
		.register("/", catchers![routes::not_found, routes::internal_error])
		.attach(Ratelimit::new())
		.attach(AttendanceFairing::new(db_clone));

	let compression = CachedCompression {
		cached_path_prefixes: vec!["/assets/".into()],
		..Default::default()
	};
	let out = out.attach(compression);

	#[cfg(feature = "cachedb")]
	let out = out.attach(SyncCache::new(db_clone2));

	let out = out.attach(UpdateStats::new(db_clone3, team_stats, auto_stats));

	out
}

/// Database setup
async fn setup_db() -> DatabaseImpl {
	let mut db = DatabaseImpl::open().await.expect("Failed to open database");
	// Ensure that an admin member is present
	let admin_member = Member {
		id: "admin".into(),
		name: "Admin".into(),
		kind: member::MemberKind::Admin,
		groups: HashSet::new(),
		password: String::new(),
		password_salt: None,
		creation_date: DateTime::UNIX_EPOCH.to_rfc2822(),
		calendar_id: String::new(),
		completed_forms: HashSet::new(),
	};
	db.create_member(admin_member)
		.await
		.expect("Failed to create admin member");

	db
}

/// Application state for Rocket
pub struct AppState {
	pub db: Arc<RwLock<DatabaseImpl>>,
	pub session_manager: Mutex<SessionManager>,
	pub password_hash: Option<Argon2<'static>>,
	pub req_client: reqwest::Client,
	pub first_client: FirstClient,
	pub statbotics_client: StatboticsClient,
	pub team_stats: Arc<RwLock<HashMap<TeamNumber, CombinedTeamStats>>>,
	pub auto_stats: Arc<RwLock<HashMap<String, AutoStats>>>,
	pub auto_images: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

pub type State = rocket::State<AppState>;
