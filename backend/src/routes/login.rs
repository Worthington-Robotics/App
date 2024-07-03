use argon2::PasswordHasher;
use password_hash::Salt;
use rocket::{form::Form, http::Status, response::content::RawHtml, FromForm};
use tracing::{error, event, span, Level};

use crate::db::Database;
use crate::State;

use super::{create_page, SessionID};

#[rocket::get("/login")]
pub async fn login() -> Result<RawHtml<String>, Status> {
	let page = create_page("Login", include_str!("pages/login.html"));
	Ok(RawHtml(page))
}

#[rocket::post("/api/auth", data = "<data>")]
pub async fn authenticate(state: &State, data: Form<AuthForm>) -> Result<String, Status> {
	let span = span!(Level::DEBUG, "Authenticating");
	let _enter = span.enter();

	// Don't allow logging in as the admin user. They can only be authenticated using the session ID given on startup.
	if data.id == "admin" {
		error!("Attempted to log in as admin");
		return Err(Status::Unauthorized);
	}

	let member = {
		let lock = state.db.lock().await;
		lock.get_member(&data.id)
	}
	.ok_or_else(|| {
		error!("Unknown member ID {}", data.id);
		Status::Unauthorized
	})?;

	// Check that passwords match
	let salt = member.password_salt.clone().unwrap_or_default();
	let Ok(salt) = Salt::from_b64(&salt) else {
		error!("Failed to create salt");
		return Err(Status::Unauthorized);
	};

	let hashed_password = if let Some(hash) = &state.password_hash {
		hash.hash_password(data.password.as_bytes(), salt)
			.map(|x| x.to_string())
	} else {
		Ok(data.password.clone())
	};
	let Ok(hashed_password) = hashed_password else {
		error!("Failed to hash password");
		return Err(Status::Unauthorized);
	};

	if hashed_password != member.password {
		event!(Level::DEBUG, "Passwords did not match");
		return Err(Status::Unauthorized);
	}

	// Create the session for them
	let session_id = {
		let mut lock = state.session_manager.lock().await;
		lock.create(&data.id)
	};

	Ok(session_id)
}

#[rocket::post("/api/logout")]
pub async fn logout(session_id: SessionID<'_>, state: &State) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Logging out");
	let _enter = span.enter();

	let mut lock = state.session_manager.lock().await;
	if lock.remove(session_id.id).is_none() {
		error!("Session that was attempted to logout did not exist");
		return Err(Status::Unauthorized);
	}

	Ok(())
}

#[derive(FromForm)]
pub struct AuthForm {
	id: String,
	password: String,
}
