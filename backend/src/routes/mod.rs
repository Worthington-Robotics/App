pub mod assets;
pub mod calendar;
pub mod login;

use std::collections::HashSet;

use argon2::PasswordHasher;
use password_hash::SaltString;
use rand::{rngs::StdRng, SeedableRng};
use rocket::response::{
	content::{RawHtml, RawJson},
	Redirect,
};
use rocket::{
	form::Form,
	http::Status,
	request::{FromRequest, Outcome},
	FromForm, Request, Responder,
};
use serde::Serialize;
use tracing::{error, event, span, Level};

use crate::{
	auth::Privilege,
	member::{Member, MemberGroup},
	State,
};
use crate::{db::Database, member::MemberKind};

#[rocket::get("/")]
pub async fn index(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Index");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.id else {
		return Ok(redirect);
	};

	let Some(requesting_member_id) = ({
		let lock = state.session_manager.lock().await;
		lock.get(session_id).map(|x| x.member.clone())
	}) else {
		error!("Unknown session ID {}", session_id);
		return Ok(redirect);
	};

	let Some(member) = ({
		let lock = state.db.lock().await;
		lock.get_member(&requesting_member_id)
	}) else {
		error!("Unknown requesting member ID {}", requesting_member_id);
		return Ok(redirect);
	};

	let page = create_page("WorBots 4145", include_str!("pages/index.html"));
	let page = page.replace("{name}", &member.name);

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

#[derive(Responder)]
pub enum PageOrRedirect {
	Page(RawHtml<String>),
	Redirect(Redirect),
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

	let mut groups = HashSet::with_capacity(member.groups.len());
	groups.extend(member.groups.clone());
	let new_member = Member {
		id: member.id.clone(),
		name: member.name.clone(),
		kind: member.kind,
		groups,
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
	groups: Vec<MemberGroup>,
	password: String,
}

/// Request guard for a session ID
pub struct SessionID<'r> {
	id: &'r str,
}

#[async_trait::async_trait]
impl<'r> FromRequest<'r> for SessionID<'r> {
	type Error = &'static str;

	async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
		let Some(session_id) = get_session_id(request) else {
			return Outcome::Error((
				Status::BadRequest,
				"Session ID not found in cookie or header",
			));
		};

		Outcome::Success(Self { id: session_id })
	}
}

/// Request guard for an optional session ID
pub struct OptionalSessionID<'r> {
	id: Option<&'r str>,
}

#[async_trait::async_trait]
impl<'r> FromRequest<'r> for OptionalSessionID<'r> {
	type Error = String;

	async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
		let session_id = get_session_id(request);

		Outcome::Success(Self { id: session_id })
	}
}

fn get_session_id<'r>(request: &'r Request) -> Option<&'r str> {
	if let Some(session_id) = request.headers().get("SessionID").next() {
		Some(session_id)
	} else {
		Some(request.cookies().get("session_id")?.value())
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
	let out = out.replace("{footer}", include_str!("components/footer.html"));

	out
}
