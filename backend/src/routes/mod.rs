pub mod assets;
pub mod login;

use argon2::PasswordHasher;
use password_hash::SaltString;
use rand::{rngs::StdRng, SeedableRng};
use rocket::{
	form::Form,
	http::Status,
	request::{FromRequest, Outcome},
	response::content::RawJson,
	FromForm, Request,
};
use serde::Serialize;
use tracing::{error, event, span, Level};

use crate::{auth::Privilege, member::Member, State};
use crate::{db::Database, member::MemberKind};

#[rocket::get("/")]
pub fn index() -> String {
	"Hello from rocket!".into()
}

#[rocket::get("/api/member/<id>")]
pub async fn get_member(
	id: &str,
	session_id: SessionID<'_>,
	state: &State,
) -> Result<RawJson<String>, Status> {
	let requesting_member_id = {
		let lock = state.session_manager.lock().await;
		lock.get(session_id.id).map(|x| x.member.clone())
	}
	.ok_or_else(|| {
		error!("Unknown session ID {}", session_id.id);
		Status::Unauthorized
	})?;

	let requesting_member = {
		let lock = state.db.lock().await;
		lock.get_member(&requesting_member_id)
	}
	.ok_or_else(|| {
		error!("Unknown requesting member ID {}", requesting_member_id);
		Status::InternalServerError
	})?;

	let desired_member = {
		let lock = state.db.lock().await;
		lock.get_member(id)
	}
	.ok_or_else(|| {
		error!("Unknown member ID {}", id);
		Status::InternalServerError
	})?;

	/*
		Check if the requesting member is allowed to be fetching this member.
		Admin members can fetch any member, but standard members can only fetch themselves
	*/
	match requesting_member.kind.get_privilege() {
		Privilege::Standard => {
			if requesting_member.id != desired_member.id {
				error!("Member attempted to fetch member other than themselves");
				return Err(Status::Unauthorized);
			}
		}
		Privilege::Elevated => {}
	}

	let out = MemberResponse {
		id: desired_member.id.clone(),
		name: desired_member.name.clone(),
		kind: desired_member.kind,
	};

	let out = serde_json::to_string(&out).map_err(|_| {
		error!("Failed to serialize member response");
		Status::InternalServerError
	})?;

	Ok(RawJson(out))
}

#[derive(Serialize)]
struct MemberResponse {
	pub id: String,
	pub name: String,
	pub kind: MemberKind,
}

#[rocket::post("/api/create_member", data = "<member>")]
pub async fn create_member(
	state: &State,
	session_id: SessionID<'_>,
	member: Form<MemberForm>,
) -> Result<String, Status> {
	let span = span!(Level::DEBUG, "Creating member");
	let _enter = span.enter();

	session_id.verify_elevated(state).await?;

	let result = if let Some(hash) = &state.password_hash {
		// Create salt
		let salt = SaltString::generate(&mut StdRng::from_entropy());
		hash.hash_password(member.password.as_bytes(), &salt.clone())
			.map(|x| (x.to_string(), Some(salt)))
	} else {
		Ok((member.password.clone(), None))
	};
	let Ok((hashed_password, salt)) = result else {
		error!("Failed to hash password");
		return Err(Status::InternalServerError);
	};

	let new_member = Member {
		id: member.id.clone(),
		name: member.name.clone(),
		kind: member.kind,
		password: hashed_password,
		password_salt: salt.map(|x| x.to_string()),
	};

	{
		let mut lock = state.db.lock().await;
		lock.create_member(new_member).map_err(|e| {
			error!("{}", e);
			Status::InternalServerError
		})?;
	}

	Ok(member.id.clone())
}

#[derive(FromForm)]
pub struct MemberForm {
	id: String,
	name: String,
	kind: MemberKind,
	password: String,
}

/// Request guard for a session ID
pub struct SessionID<'r> {
	id: &'r str,
}

#[async_trait::async_trait]
impl<'r> FromRequest<'r> for SessionID<'r> {
	type Error = String;

	async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
		let session_id = if let Some(session_id) = request.headers().get("SessionID").next() {
			session_id
		} else {
			let Some(session_id) = request.cookies().get("session_id") else {
				return Outcome::Error((
					Status::BadRequest,
					"Session ID not found in cookie or header".into(),
				));
			};

			session_id.value()
		};

		Outcome::Success(SessionID { id: session_id })
	}
}

impl<'r> SessionID<'r> {
	/// Verify that the session ID is valid and that the requesting member has elevated permissions
	pub async fn verify_elevated(&self, state: &State) -> Result<(), Status> {
		let span = span!(Level::DEBUG, "Verifying session elevated permissions");
		let _enter = span.enter();

		let requesting_member_id = {
			let lock = state.session_manager.lock().await;
			lock.get(self.id).map(|x| x.member.clone())
		}
		.ok_or_else(|| {
			error!("Unknown session ID {}", self.id);
			Status::Unauthorized
		})?;

		let requesting_member = {
			let lock = state.db.lock().await;
			lock.get_member(&requesting_member_id)
		}
		.ok_or_else(|| {
			error!("Unknown requesting member ID {}", requesting_member_id);
			Status::Unauthorized
		})?;

		if requesting_member.kind.get_privilege() != Privilege::Elevated {
			event!(
				Level::DEBUG,
				"Requesting member does not have high enough permissions"
			);
			return Err(Status::Unauthorized);
		}

		Ok(())
	}
}

pub fn create_page(title: &str, body: &str) -> String {
	static HEAD: &str = include_str!("pages/head.html");
	let head = HEAD.replace("{title}", title);
	let out = head.replace("{body}", body);

	out
}

// /// Error for API responses
// #[derive(thiserror::Error, Debug, Responder)]
// enum Error {
// 	/// An unknown error with just a status
// 	Status(Status),
// }

// impl<'r, 'o> Responder<'r, 'o> for Error {
// 	fn respond_to(self, request: &'r Request<'_>) -> rocket::response::Result<'o> {
// 		match self {
// 			Self::Status(status) => Err(status),
// 		}
// 	}
// }
