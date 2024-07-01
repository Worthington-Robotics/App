use db::json::JSONDatabase;
use rocket::{routes, tokio::sync::Mutex};

mod db;
mod member;
mod routes;

#[rocket::launch]
fn rocket() -> _ {
	println!("Starting server...");

	rocket::build().mount("/", routes![routes::index])
}

/// Application state for Rocket
pub struct State {
	pub db: Mutex<JSONDatabase>,
}
