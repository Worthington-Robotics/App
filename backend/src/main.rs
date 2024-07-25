use std::fmt::Display;

use argon2::Argon2;
use auth::SessionManager;
use base64::{
	engine::{GeneralPurpose, GeneralPurposeConfig},
	Engine,
};
use chrono::{DateTime, Offset, TimeZone};
use db::{json::JSONDatabase, Database};
use rand::{rngs::StdRng, RngCore, SeedableRng};
use rocket::{catchers, routes, tokio::sync::Mutex};

mod announcements;
mod auth;
mod db;
mod events;
mod forms;
mod member;
mod routes;
mod util;

#[rocket::launch]
fn rocket() -> _ {
	println!("Starting server...");
	let subscriber = tracing_subscriber::FmtSubscriber::new();
	tracing::subscriber::set_global_default(subscriber)
		.expect("Failed to set global tracing subscriber");

	let mut session_manager = SessionManager::new();

	let session_id = session_manager.create("admin");
	println!("Session ID: {session_id}");

	let db = JSONDatabase::open().expect("Failed to open database");
	db.debug();

	// Load password hash
	let params = argon2::Params::new(15000, 2, 1, None).expect("Failed to build Argon2 parameters");

	let password_hash = Some(Argon2::new(
		argon2::Algorithm::Argon2id,
		argon2::Version::V0x13,
		params,
	));

	let state = AppState {
		db: Mutex::new(db),
		session_manager: Mutex::new(session_manager),
		password_hash,
	};

	rocket::build()
		.manage(state)
		.mount(
			"/",
			routes![
				routes::index,
				routes::members::get_member,
				routes::members::create_member,
				routes::members::member_list,
				routes::assets::favicon,
				routes::assets::main_css,
				routes::assets::logo,
				routes::assets::rockwell,
				routes::assets::icon_home,
				routes::assets::icon_clock,
				routes::assets::icon_plus,
				routes::assets::icon_mail,
				routes::assets::icon_edit,
				routes::login::login,
				routes::login::authenticate,
				routes::login::logout,
				routes::calendar::calendar,
				routes::calendar::create_event,
				routes::calendar::create_event_api,
				routes::inbox::inbox,
			],
		)
		.register("/", catchers![routes::not_found, routes::internal_error])
}

/// Application state for Rocket
pub struct AppState {
	pub db: Mutex<JSONDatabase>,
	pub session_manager: Mutex<SessionManager>,
	pub password_hash: Option<Argon2<'static>>,
}

pub type State = rocket::State<AppState>;

/// Generate the ID for something like an event
fn generate_id() -> String {
	let mut rng = StdRng::from_entropy();
	let base64 = GeneralPurpose::new(&base64::alphabet::STANDARD, GeneralPurposeConfig::new());
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
