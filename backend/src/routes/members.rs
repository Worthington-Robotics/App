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
use strum::IntoEnumIterator;
use tracing::{error, span, Level};

use crate::routes::OptionalSessionID;
use crate::util::ToDropdown;
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

	let (hashed_password, salt) = if let Some(password) = &member.password {
		let result = if let Some(hash) = &state.password_hash {
			// Create salt
			let salt = SaltString::generate(&mut StdRng::from_entropy());
			hash.hash_password(password.as_bytes(), &salt.clone())
				.map(|x| (x.to_string(), Some(salt)))
		} else {
			Ok((password.clone(), None))
		};
		let Ok((hashed_password, salt)) = result else {
			error!("Failed to hash password");
			return Err(Status::InternalServerError);
		};

		(Some(hashed_password), salt)
	} else {
		(None, None)
	};

	// Don't replace the password for an existing member if it wasn't specified in the form
	let hashed_password = if let Some(hashed_password) = hashed_password {
		hashed_password
	} else {
		let existing_member = state.db.lock().await.get_member(&member.id);
		let Some(existing_member) = existing_member else {
			error!("Password not given when there is no existing member");
			return Err(Status::Unauthorized);
		};
		existing_member.password.clone()
	};

	let groups = serde_json::from_str(&member.groups);
	let Ok(groups) = groups else {
		error!("Failed to deserialize groups: {}", member.groups);
		return Err(Status::BadRequest);
	};
	let groups: Vec<String> = groups;
	let groups = groups
		.into_iter()
		.map(|x| match x.as_str() {
			"Member" => MemberGroup::Member,
			"New Member" => MemberGroup::NewMember,
			"Pit Crew" => MemberGroup::PitCrew,
			"Lead" => MemberGroup::Lead,
			"President" => MemberGroup::President,
			"Coach" => MemberGroup::Coach,
			"Mentor" => MemberGroup::Mentor,
			_ => MemberGroup::Member,
		})
		.collect();

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
	groups: String,
	password: Option<String>,
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

	let new_button = format!(
		"<a href=\"/create_member\">{}</a>",
		include_str!("components/new.min.html")
	);

	let page = page.replace("{{add-member}}", &new_button);

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

#[rocket::get("/create_member?<id>")]
pub async fn create_member_page(
	id: Option<&str>,
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Create member page");
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

	let Some(requesting_member) = ({
		let lock = state.db.lock().await;
		lock.get_member(&requesting_member_id)
	}) else {
		error!("Unknown requesting member ID {}", requesting_member_id);
		return Ok(redirect);
	};

	if requesting_member.kind.get_privilege() != Privilege::Elevated {
		error!("Invalid permissions");
		return Ok(redirect);
	}

	let lock = state.db.lock().await;
	let member = if let Some(id) = id {
		// We are editing an existing member
		lock.get_member(id).ok_or_else(|| {
			error!("Member does not exist: {}", id);
			Status::InternalServerError
		})?
	} else {
		// We are making a new member
		Member {
			id: String::new(),
			name: String::new(),
			kind: MemberKind::Standard,
			groups: HashSet::new(),
			password: String::new(),
			password_salt: None,
		}
	};

	let page = include_str!("pages/create_member.html");
	let page = page.replace("{{id}}", &member.id);
	let page = page.replace("{{name}}", &member.name);

	// Create dropdown options
	let page = page.replace(
		"{{kind-options}}",
		&MemberKind::create_options(Some(&member.kind)),
	);

	// Generate group checkboxes
	let mut groups_string = String::new();
	let mut available_groups = Vec::new();
	for group in MemberGroup::iter() {
		let checked = member.groups.contains(&group);
		available_groups.push((
			group.to_dropdown(),
			format!(
				"<div class=\"group-label\">{}</div>",
				group.to_plural_string().to_string()
			),
			checked,
		));
	}

	// Create password field only if the member doesn't already exist
	let password_field = if id.is_none() {
		"<input type=password name=password id=password-field class=create-member-field placeholder=\"Enter member password...\" autocomplete=new-password />"
	} else {
		""
	};
	let page = page.replace("{{password}}", password_field);

	for (i, (group, group_pretty, is_checked)) in available_groups.into_iter().enumerate() {
		let label = format!("<label for=\"{group}\">{group_pretty}</label>");
		let checked_string = if is_checked { " checked" } else { "" };
		let checkbox = format!("<input type=\"checkbox\" name=\"{group}\" id=\"group-checkbox-{i}\" {checked_string} />");

		let group = format!("<div class=\"cont group-checkbox\">{label}{checkbox}</div>");

		groups_string.push_str(&group);
	}
	let page = page.replace("{{groups}}", &groups_string);

	let page = create_page("Create Member", &page);

	Ok(PageOrRedirect::Page(RawHtml(page)))
}
