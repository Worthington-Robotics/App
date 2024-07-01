use rocket::{
	http::Status,
	request::{FromRequest, Outcome},
	response::content::RawJson,
	Request,
};
use serde::Serialize;
use tracing::error;

use crate::{auth::Privilege, State};
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
		lock.get_session(session_id.id).map(|x| x.member.clone())
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

/// Request guard for a session ID
pub struct SessionID<'r> {
	id: &'r str,
}

#[async_trait::async_trait]
impl<'r> FromRequest<'r> for SessionID<'r> {
	type Error = String;

	async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
		let session_id = if let Some(session) = request.cookies().get("SESSION") {
			session.value()
		} else {
			let Some(session_id) = request.headers().get("SessionID").next() else {
				return Outcome::Error((
					Status::BadRequest,
					"Session ID not found in cookie or header".into(),
				));
			};

			session_id
		};

		Outcome::Success(SessionID { id: session_id })
	}
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
