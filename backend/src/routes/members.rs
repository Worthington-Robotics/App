use std::cmp::Reverse;
use std::collections::HashSet;
use std::ops::Deref;

use argon2::PasswordHasher;
use chrono::{DateTime, Utc};
use chrono_tz::US::Eastern;
use itertools::Itertools;
use password_hash::SaltString;
use rand::{rngs::StdRng, SeedableRng};
use rocket::response::content::{RawHtml, RawJson};
use rocket::response::Redirect;
use rocket::{form::Form, http::Status, FromForm};
use serde::Serialize;
use strum::IntoEnumIterator;
use tracing::{error, span, Level};

use crate::attendance::get_attendance_stats;
use crate::events::Event;
use crate::routes::OptionalSessionID;
use crate::util::ToDropdown;
use crate::util::{generate_id, render_date};
use crate::{
	auth::Privilege,
	member::{Member, MemberGroup},
	routes::SessionID,
	State,
};
use crate::{db::Database, member::MemberKind};

use super::{create_page, PageOrRedirect, Scope};

#[rocket::get("/api/member/<id>")]
pub async fn get_member(
	id: &str,
	session_id: SessionID<'_>,
	state: &State,
) -> Result<RawJson<String>, Status> {
	let requesting_member = session_id.get_requesting_member(state).await?;

	let desired_member = {
		let lock = state.db.lock().await;
		lock.get_member(id).await
	}
	.map_err(|e| {
		error!("Failed to get member from database: {e}");
		Status::InternalServerError
	})?
	.ok_or_else(|| {
		error!("Unknown member ID {}", requesting_member.id);
		Status::NotFound
	})?;

	/*
		Check if the requesting member is allowed to be fetching this member.
		Admin members can fetch any member, but standard members can only fetch themselves
	*/
	match requesting_member.kind.get_privilege() {
		Privilege::Standard => {
			if requesting_member.id != desired_member.id {
				error!("Member attempted to fetch member other than themselves");
				// Prevent user enumeration
				return Err(Status::NotFound);
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

	if !member.id.is_ascii() || member.id.contains(' ') {
		error!("Invalid member ID");
		return Err(Status::BadRequest);
	}

	let existing_member = state
		.db
		.lock()
		.await
		.get_member(&member.id)
		.await
		.map_err(|e| {
			error!("Failed to get member from database: {e}");
			Status::InternalServerError
		})?;

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

	// Don't replace the password or salt for an existing member if it wasn't specified in the form
	let (hashed_password, salt) = if let Some(hashed_password) = hashed_password {
		(hashed_password, salt.map(|x| x.to_string()))
	} else {
		let Some(existing_member) = &existing_member else {
			error!("Password not given when there is no existing member");
			return Err(Status::Unauthorized);
		};
		(
			existing_member.password.clone(),
			existing_member.password_salt.clone(),
		)
	};

	let groups = serde_json::from_str(&member.groups);
	let Ok(groups) = groups else {
		error!("Failed to deserialize groups: {}", member.groups);
		return Err(Status::BadRequest);
	};
	let groups: Vec<String> = groups;
	let groups = groups
		.into_iter()
		.map(|x| {
			MemberGroup::from_dropdown(&x).unwrap_or_else(|| {
				error!("Failed to parse member group {x}");
				MemberGroup::Member
			})
		})
		.collect();

	// Don't replace the creation date for existing members either
	let creation_date = if let Some(existing_member) = &existing_member {
		existing_member.creation_date.clone()
	} else {
		Utc::now().to_rfc2822()
	};

	let calendar_id = existing_member
		.map(|x| x.calendar_id.clone())
		.unwrap_or(generate_id());

	let new_member = Member {
		id: member.id.clone(),
		name: member.name.clone(),
		kind: member.kind,
		groups,
		password: hashed_password,
		password_salt: salt.map(|x| x.to_string()),
		creation_date,
		calendar_id,
	};

	{
		let mut lock = state.db.lock().await;
		lock.create_member(new_member).await.map_err(|e| {
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
	let span = span!(Level::DEBUG, "Member list");
	let _enter = span.enter();

	if session_id.verify_elevated(state).await.is_err() {
		error!("Member tried to access member list without valid permissions");
		return Ok(PageOrRedirect::Redirect(Redirect::to("/login")));
	}

	let page = include_str!("pages/members/member_list.min.html");
	let page = create_page("Members", page, Some(Scope::Home));

	let mut member_list = String::new();
	for member in state
		.db
		.lock()
		.await
		.get_members()
		.await
		.map_err(|e| {
			error!("Failed to get members from database: {e}");
			Status::InternalServerError
		})?
		.sorted_by_key(|x| x.name.clone())
	{
		member_list.push_str(&render_member_entry(&member));
	}
	let page = page.replace("{{members}}", &member_list);

	let new_button = format!(
		"<a href=\"/create_member\">{}</a>",
		include_str!("components/ui/new.min.html")
	);

	let page = page.replace("{{add-member}}", &new_button);

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

fn render_member_entry(member: &Member) -> String {
	let element = include_str!("components/member_entry.min.html");
	let element = element.replace("{{name}}", &member.name);
	let kind = if member.kind == MemberKind::Admin {
		format!(
			"<div class=\"mem-kind\">{}</div><div class=\"dot\"></div>",
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
	let element = element.replace("{{id}}", &member.id);

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
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.verify_elevated(state).await.is_err() {
		return Ok(redirect);
	};

	let member = if let Some(id) = id {
		let lock = state.db.lock().await;
		// We are editing an existing member
		lock.get_member(id)
			.await
			.map_err(|e| {
				error!("Failed to get member from database: {e}");
				Status::InternalServerError
			})?
			.ok_or_else(|| {
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
			creation_date: Utc::now().to_rfc2822(),
			calendar_id: String::new(),
		}
	};

	let page = include_str!("pages/members/create_member.min.html");
	let page = page.replace("{{name}}", &format!("\"{}\"", member.name));

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

	for (i, (group, group_pretty, is_checked)) in available_groups.into_iter().enumerate() {
		let label = format!("<label for=\"{group}\">{group_pretty}</label>");
		let checked_string = if is_checked { " checked" } else { "" };
		let checkbox = format!("<input type=\"checkbox\" name=\"{group}\" id=\"group-checkbox-{i}\" {checked_string} />");

		let group = format!("<div class=\"cont group-checkbox\">{label}{checkbox}</div>");

		groups_string.push_str(&group);
	}
	let page = page.replace("{{groups}}", &groups_string);

	// Create ID field and password field only if the member doesn't already exist.
	// Replace the value for the ID in the JavaScript based on whether it is new or not
	let (id_field, id, password_field) = if let Some(id) = id {
		("", format!("\"{id}\""), "")
	} else {
		(
			"<input type=text name=id id=id-field class=create-member-field placeholder=\"Enter member username...\" />",
			"document.getElementById(\"id-field\").value".to_string(),
			"<input type=password name=password id=password-field class=create-member-field placeholder=\"Enter member password...\" autocomplete=new-password />"
		)
	};
	let page = page.replace("{{id-field}}", id_field);
	let page = page.replace("__id__", &id);
	let page = page.replace("{{password}}", password_field);

	let page = create_page("Create Member", &page, Some(Scope::Home));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

#[rocket::get("/member/<id>")]
pub async fn member_details(
	id: &str,
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Member details page");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.verify_elevated(state).await.is_err() {
		// Prevent user enumeration
		return Err(Status::NotFound);
	};

	let lock = state.db.lock().await;
	let member = lock
		.get_member(id)
		.await
		.map_err(|e| {
			error!("Failed to get member from database: {e}");
			Status::InternalServerError
		})?
		.ok_or_else(|| {
			error!("Member does not exist: {}", id);
			Status::NotFound
		})?;

	let page = include_str!("pages/members/details.min.html");
	let page = page.replace("{{id}}", &member.id);
	let page = page.replace("{{name}}", &member.name);
	let page = page.replace("{{kind}}", &member.kind.to_string());

	let date = if let Ok(date) = DateTime::parse_from_rfc2822(&member.creation_date) {
		render_date(date.with_timezone(&Eastern))
	} else {
		error!("Failed to parse date");
		"Invalid date".into()
	};
	let page = page.replace("{{creation-date}}", &date);

	let page = page.replace("{{edit}}", include_str!("components/ui/edit.min.html"));
	let page = page.replace("{{delete}}", include_str!("components/ui/delete.min.html"));

	// Attendance stats
	let (season_attendance, total_attendance) = get_attendance_stats(&member, lock.deref())
		.await
		.map_err(|e| {
		error!("Failed to get attendance stats: {e}");
		Status::InternalServerError
	})?;
	let page = page.replace("{{season-ratio}}", &season_attendance.format_ratio());
	let page = page.replace("{{season-percentage}}", &season_attendance.format_percent());
	let page = page.replace("{{season-average}}", &season_attendance.format_average());
	let page = page.replace(
		"{{season-missed}}",
		&render_missed_events(&season_attendance.absences),
	);
	let page = page.replace("{{total-ratio}}", &total_attendance.format_ratio());
	let page = page.replace("{{total-percentage}}", &total_attendance.format_percent());
	let page = page.replace("{{total-average}}", &total_attendance.format_average());
	let page = page.replace(
		"{{total-missed}}",
		&render_missed_events(&total_attendance.absences),
	);

	let page = create_page("Member Details", &page, Some(Scope::Home));

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

fn render_missed_events(events: &[Event]) -> String {
	let mut out = String::new();
	for event in events {
		let date = match DateTime::parse_from_rfc2822(&event.date) {
			Ok(date) => render_date(date),
			Err(e) => {
				error!("Failed to parse date for event {}: {e}", event.id);
				"Invalid date".into()
			}
		};
		let component = format!("<div class=\"item round\">{} - {}</div>", event.name, date);

		out.push_str(&component);
	}

	out
}

#[rocket::delete("/api/delete_member/<id>")]
pub async fn delete_member(
	state: &State,
	session_id: SessionID<'_>,
	id: &str,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Deleting member");
	let _enter = span.enter();

	session_id.verify_elevated(state).await?;

	let mut lock = state.db.lock().await;
	if !lock.member_exists(id).await.map_err(|e| {
		error!("Failed to get member from database: {e}");
		Status::InternalServerError
	})? {
		error!("Attempted to delete non-existent member {id}");
		return Err(Status::NotFound);
	}

	if let Err(e) = lock.delete_member(id).await {
		error!("Failed to delete member {id} in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}
