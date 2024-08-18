use std::{collections::HashSet, sync::Arc};

use argon2::Argon2;
use attendance::AttendanceFairing;
use auth::SessionManager;
use chrono::DateTime;
#[cfg(feature = "cachedb")]
use db::cached::SyncCache;
use db::{Database, DatabaseImpl};
use dotenv::dotenv;
use member::Member;
use rocket::{catchers, routes, tokio::sync::Mutex};
use routes::Ratelimit;

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
	};
	db.create_member(admin_member)
		.await
		.expect("Failed to create admin member");

	// Load password hash
	let params = argon2::Params::new(15000, 2, 1, None).expect("Failed to build Argon2 parameters");

	let password_hash = Some(Argon2::new(
		argon2::Algorithm::Argon2id,
		argon2::Version::V0x13,
		params,
	));

	let state = AppState {
		db: Arc::new(Mutex::new(db)),
		session_manager: Mutex::new(session_manager),
		password_hash,
	};

	let db_clone = state.db.clone();
	#[cfg(feature = "cachedb")]
	let db_clone2 = state.db.clone();

	let out = rocket::build()
		.manage(state)
		.mount(
			"/",
			routes![
				routes::index,
				routes::members::get_member,
				routes::members::create_member,
				routes::members::member_list,
				routes::members::create_member_page,
				routes::members::member_details,
				routes::members::delete_member,
				routes::assets::favicon,
				routes::assets::main_css,
				routes::assets::static_css,
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

	#[cfg(feature = "cachedb")]
	let out = out.attach(SyncCache::new(db_clone2));

	out
}

/// Application state for Rocket
pub struct AppState {
	pub db: Arc<Mutex<DatabaseImpl>>,
	pub session_manager: Mutex<SessionManager>,
	pub password_hash: Option<Argon2<'static>>,
}

pub type State = rocket::State<AppState>;
