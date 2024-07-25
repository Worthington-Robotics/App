pub mod assets;
pub mod calendar;
pub mod inbox;
pub mod login;
pub mod members;

use rocket::response::{content::RawHtml, Redirect};
use rocket::{
	http::Status,
	request::{FromRequest, Outcome},
	Request, Responder,
};
use tracing::{error, event, span, Level};

use crate::db::Database;
use crate::{auth::Privilege, State};

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

	let page = create_page("WorBots 4145", include_str!("pages/index.min.html"));
	let page = page.replace("{{name}}", &member.name);
	let admin_panel = if member.kind.get_privilege() == Privilege::Elevated {
		include_str!("components/admin_panel.min.html")
	} else {
		""
	};
	let page = page.replace("{{admin_panel}}", admin_panel);

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

#[derive(Responder)]
pub enum PageOrRedirect {
	Page(RawHtml<String>),
	Redirect(Redirect),
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
	let head = HEAD.replace("{{title}}", &format!("{title} - WorBots"));
	let out = head.replace("{{body}}", body);
	let out = out.replace("{{footer}}", include_str!("components/footer.min.html"));
	let out = out.replace(
		"{{worbots-header}}",
		include_str!("components/worbots-header.min.html"),
	);

	out
}

#[rocket::catch(404)]
pub fn not_found() -> RawHtml<String> {
	RawHtml(create_page("Not Found", include_str!("pages/404.min.html")))
}

#[rocket::catch(500)]
pub fn internal_error() -> RawHtml<String> {
	RawHtml(create_page(
		"Internal Error",
		include_str!("pages/500.min.html"),
	))
}
