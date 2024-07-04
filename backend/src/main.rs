use argon2::Argon2;
use auth::SessionManager;
use db::{json::JSONDatabase, Database};
use rocket::{routes, tokio::sync::Mutex};

mod auth;
mod db;
mod events;
mod member;
mod routes;

#[rocket::launch]
fn rocket() -> _ {
	println!("Starting server...");

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

	rocket::build().manage(state).mount(
		"/",
		routes![
			routes::index,
			routes::get_member,
			routes::create_member,
			routes::assets::favicon,
			routes::assets::main_css,
			routes::assets::rockwell,
			routes::assets::icon_home,
			routes::assets::icon_clock,
			routes::login::login,
			routes::login::authenticate,
			routes::login::logout,
			routes::calendar::calendar,
		],
	)
}

/// Application state for Rocket
pub struct AppState {
	pub db: Mutex<JSONDatabase>,
	pub session_manager: Mutex<SessionManager>,
	pub password_hash: Option<Argon2<'static>>,
}

pub type State = rocket::State<AppState>;
