use auth::SessionManager;
use db::{json::JSONDatabase, Database};
use rocket::{routes, tokio::sync::Mutex};

mod auth;
mod db;
mod member;
mod routes;

#[rocket::launch]
fn rocket() -> _ {
	println!("Starting server...");

	let mut session_manager = SessionManager::new();

	let session_id = session_manager.new_session("carbon");
	println!("Session ID: {session_id}");

	let db = JSONDatabase::open().expect("Failed to open database");
	db.debug();

	let state = AppState {
		db: Mutex::new(db),
		session_manager: Mutex::new(session_manager),
	};

	rocket::build()
		.manage(state)
		.mount("/", routes![routes::index, routes::get_member])
}

/// Application state for Rocket
pub struct AppState {
	pub db: Mutex<JSONDatabase>,
	pub session_manager: Mutex<SessionManager>,
}

pub type State = rocket::State<AppState>;
