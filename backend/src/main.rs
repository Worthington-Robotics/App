use std::{collections::HashSet, fmt::Display, sync::Arc};

use argon2::Argon2;
use attendance::AttendanceFairing;
use auth::SessionManager;
use base64::{
	engine::{GeneralPurpose, GeneralPurposeConfig},
	Engine,
};
use chrono::{DateTime, Offset, TimeZone};
use db::{Database, DatabaseImpl};
use dotenv::dotenv;
use member::Member;
use rand::{rngs::StdRng, RngCore, SeedableRng};
use rocket::{catchers, routes, tokio::sync::Mutex};
use routes::Ratelimit;

mod announcements;
mod attendance;
mod auth;
mod db;
mod events;
mod forms;
mod member;
mod routes;
mod util;

#[rocket::launch]
async fn rocket() -> _ {
	println!("Starting server...");
	let subscriber = tracing_subscriber::FmtSubscriber::new();
	tracing::subscriber::set_global_default(subscriber)
		.expect("Failed to set global tracing subscriber");

	dotenv().expect("Failed to load environment variables");

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

	rocket::build()
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
				routes::login::login,
				routes::login::authenticate,
				routes::login::logout,
				routes::calendar::calendar,
				routes::calendar::create_event,
				routes::calendar::create_event_api,
				routes::inbox::inbox,
				routes::attendance::attend,
				routes::attendance::unattend,
			],
		)
		.register("/", catchers![routes::not_found, routes::internal_error])
		.attach(Ratelimit::new())
		.attach(AttendanceFairing::new(db_clone))
}

/// Application state for Rocket
pub struct AppState {
	pub db: Arc<Mutex<DatabaseImpl>>,
	pub session_manager: Mutex<SessionManager>,
	pub password_hash: Option<Argon2<'static>>,
}

pub type State = rocket::State<AppState>;

/// Generate the ID for something like an event
fn generate_id() -> String {
	let mut rng = StdRng::from_entropy();
	let base64 = GeneralPurpose::new(&base64::alphabet::URL_SAFE, GeneralPurposeConfig::new());
	const LENGTH: usize = 32;
	let mut out = [0; LENGTH];
	for i in 0..LENGTH {
		out[i] = rng.next_u64() as u8;
	}

	base64.encode(out)
}

/// Render a nice date
fn render_date<T: TimeZone + Offset>(date: DateTime<T>) -> String
where
	T::Offset: Display,
{
	date.format("%a %B %d, %I:%M %p")
		.to_string()
		.replace(":00", "")
		.replace(" 0", " ")
}
