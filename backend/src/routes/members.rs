use std::cmp::Reverse;
use std::collections::HashSet;

use argon2::PasswordHasher;
use itertools::Itertools;
use password_hash::SaltString;
use rand::{rngs::StdRng, SeedableRng};
use rocket::response::content::{RawHtml, RawJson};
use rocket::response::Redirect;
use rocket::{form::Form, http::Status, FromForm};
use serde::Serialize;
use tracing::{error, span, Level};

use crate::{
	auth::Privilege,
	member::{Member, MemberGroup},
	routes::SessionID,
	State,
};
use crate::{db::Database, member::MemberKind};

use super::{create_page, PageOrRedirect};

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

#[rocket::get("/member_list")]
pub async fn member_list(
	state: &State,
	session_id: SessionID<'_>,
) -> Result<PageOrRedirect, Status> {
	if session_id.verify_elevated(state).await.is_err() {
		error!("Member tried to access member list without valid permissions");
		return Ok(PageOrRedirect::Redirect(Redirect::to("/login")));
	}

	let page = include_str!("pages/member_list.min.html");
	let page = create_page("Members", page);

	let mut member_list = String::new();
	for member in state
		.db
		.lock()
		.await
		.get_members()
		.sorted_by_key(|x| &x.name)
	{
		member_list.push_str(&render_member_entry(member));
	}
	let page = page.replace("{{members}}", &member_list);

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

fn render_member_entry(member: &Member) -> String {
	let element = include_str!("components/member_entry.min.html");
	let element = element.replace("{{name}}", &member.name);
	let kind = if member.kind == MemberKind::Admin {
		format!(
			"<div class=\"member-kind\">{}</div><div class=\"dot\"></div>",
			member.kind
		)
	} else {
		String::new()
	};
	let element = element.replace("{{kind}}", &kind);

	let mut groups = String::new();
	for (i, group) in member
		.groups
		.iter()
		.sorted_by_key(|x| Reverse(*x))
		.enumerate()
	{
		groups.push_str(&format!("<div class=\"member-group\">{group}</div>"));
		if i != member.groups.len() - 1 {
			groups.push_str("<div class=\"dot\"></div>");
		}
	}
	let element = element.replace("{{groups}}", &groups);

	element
}
