pub mod assets;
pub mod attendance;
pub mod calendar;
pub mod inbox;
pub mod login;
pub mod members;
pub mod scouting;
pub mod settings;
pub mod tasks;

use std::collections::HashMap;
use std::net::IpAddr;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use attendance::create_attendance_panel;
use rocket::fairing::{Fairing, Info, Kind};
use rocket::http::{ContentType, Header};
use rocket::response::{content::RawHtml, Redirect};
use rocket::tokio::sync::Mutex;
use rocket::{
	http::Status,
	request::{FromRequest, Outcome},
	response::Responder as ResponderTrait,
	Request, Responder,
};
use rocket::{Data, Orbit, Response, Rocket};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use tracing::{error, event, span, Level};

use crate::announcements::count_unread_announcements;
use crate::db::{Database, DatabaseImpl};
use crate::forms::Form;
use crate::member::Member;
use crate::State;

#[rocket::get("/")]
pub async fn index(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<CORSAllow<PageOrRedirect>, Status> {
	let span = span!(Level::DEBUG, "Index");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.id else {
		return Ok(CORSAllow(redirect));
	};

	let Some(requesting_member_id) = ({
		let lock = state.session_manager.lock().await;
		lock.get(session_id).map(|x| x.member.clone())
	}) else {
		error!("Unknown session ID {}", session_id);
		return Ok(CORSAllow(redirect));
	};

	let lock = state.db.read().await;

	let Some(member) = ({
		lock.get_member(&requesting_member_id).await.map_err(|e| {
			error!("Failed to get member from database: {e}");
			Status::InternalServerError
		})?
	}) else {
		error!("Unknown requesting member ID {}", requesting_member_id);
		return Ok(CORSAllow(redirect));
	};

	let page = create_page(
		"WorBots 4145",
		include_str!("pages/index.min.html"),
		Some(Scope::Home),
	);
	let page = page.replace("{{name}}", &member.name);
	let admin_panel = if member.is_elevated() {
		include_str!("components/admin_panel.min.html")
	} else {
		""
	};
	let page = page.replace("{{admin-panel}}", admin_panel);
	let attendance_panel = create_attendance_panel(&member, lock.deref())
		.await
		.map_err(|e| {
			error!("Failed to create attendance panel: {e}");
			Status::InternalServerError
		})?;
	let page = page.replace("{{attendance-panel}}", &attendance_panel);

	// Add notices
	let notices = render_notices(&member, lock.deref()).await;
	let page = page.replace("{{notices}}", &notices);

	Ok(CORSAllow(PageOrRedirect::Page(RawHtml(page))))
}

async fn render_notices(member: &Member, db: &DatabaseImpl) -> String {
	let mut notices_str = String::new();

	match count_unread_announcements(&member, db).await {
		Ok(announcements) => {
			if announcements > 0 {
				let word = if announcements == 1 {
					"announcement"
				} else {
					"announcements"
				};

				notices_str.push_str(&render_notice(&format!(
					"<a class=nolink href=/inbox>You have {announcements} unread {word}</a>"
				)));
			}
		}
		Err(e) => {
			error!("Failed to count unread announcements: {e:#}");
		}
	}

	if !member.completed_forms.contains(&Form::ConsentRelease) {
		notices_str.push_str(&render_notice(
			"You have not completed your consent-release form",
		));
	}

	if !member.completed_forms.contains(&Form::TeamFees) {
		notices_str.push_str(&render_notice("You have not submitted your team fees"));
	}

	if notices_str.is_empty() {
		String::new()
	} else {
		format!("<div class=\"cont col round\" id=notices-cont><h3>Notices</h3>{notices_str}</div>")
	}
}

fn render_notice(text: &str) -> String {
	format!("<div class=\"round notice\"><div class=cont><div class=notice-bullet></div></div>{text}</div>")
}

#[rocket::get("/admin")]
pub async fn admin(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Admin Page");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));

	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.verify_elevated(state).await.is_err() {
		return Ok(redirect);
	}

	let page = include_str!("pages/admin.min.html");
	let page = create_page("Administration", page, Some(Scope::Home));

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

impl<'r> OptionalSessionID<'r> {
	/// Convert this optional session ID to a SessionID, optionally
	pub fn to_session_id(self) -> Option<SessionID<'r>> {
		self.id.map(|id| SessionID { id })
	}
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
	/// Get the requesting member. This will lock the database mutex
	pub async fn get_requesting_member(&self, state: &State) -> Result<Member, Status> {
		let span = span!(Level::DEBUG, "Getting requesting member");
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
			let lock = state.db.read().await;
			lock.get_member(&requesting_member_id).await
		}
		.map_err(|e| {
			error!("Failed to get member from database: {e}");
			Status::Unauthorized
		})?
		.ok_or_else(|| {
			error!("Unknown requesting member ID {}", requesting_member_id);
			Status::Unauthorized
		})?;

		Ok(requesting_member)
	}

	/// Verify that the session ID is valid and that the requesting member has elevated permissions. This will lock the database mutex
	pub async fn verify_elevated(&self, state: &State) -> Result<(), Status> {
		let span = span!(Level::DEBUG, "Verifying session elevated permissions");
		let _enter = span.enter();

		let requesting_member = self.get_requesting_member(state).await?;

		if !requesting_member.is_elevated() {
			event!(
				Level::DEBUG,
				"Requesting member does not have high enough permissions"
			);
			return Err(Status::Unauthorized);
		}

		Ok(())
	}
}

pub fn create_page(title: &str, body: &str, scope: Option<Scope>) -> String {
	static HEAD: &str = include_str!("components/util/head.min.html");
	let head = HEAD.replace("{{title}}", &format!("{title} - WorBots"));
	let out = head.replace("{{body}}", body);

	// Render the footer
	let mut footer = include_str!("components/util/footer.min.html").to_string();
	for (i, variant) in Scope::iter().enumerate() {
		let class = if scope.is_some_and(|x| x == variant) {
			"ft-sel"
		} else {
			""
		};

		footer = footer.replace(&format!("{{{{sel{i}}}}}"), class);
	}
	let out = out.replace("{{footer}}", &footer);

	// Extra optional elements that can be included
	let out = out.replace(
		"{{worbots-header}}",
		include_str!("components/util/worbots-header.min.html"),
	);
	let out = out.replace("{{error}}", include_str!("components/util/error.min.html"));
	let out = out.replace(
		"{{prompt}}",
		include_str!("components/util/prompt.min.html"),
	);
	let out = out.replace(
		"{{stat-popup}}",
		include_str!("components/util/stat_popup.min.html"),
	);

	out
}

/// Different scopes of the application, for rendering the footer
#[derive(EnumIter, PartialEq, Clone, Copy)]
pub enum Scope {
	Scouting,
	Announcements,
	Home,
	Events,
	Todo,
}

#[rocket::catch(404)]
pub fn not_found() -> RawHtml<String> {
	RawHtml(create_page(
		"Not Found",
		include_str!("pages/errors/404.min.html"),
		None,
	))
}

#[rocket::catch(500)]
pub fn internal_error() -> RawHtml<String> {
	RawHtml(create_page(
		"Internal Error",
		include_str!("pages/errors/500.min.html"),
		None,
	))
}

/// Rocket fairing for implementing a ratelimit
pub struct Ratelimit {
	request_counts: Arc<Mutex<HashMap<IpAddr, u16>>>,
}

impl Ratelimit {
	pub fn new() -> Self {
		Self {
			request_counts: Arc::new(Mutex::new(HashMap::new())),
		}
	}
}

/// Ratelimit for requests per minute
pub const RATELIMIT: u16 = 200;

#[async_trait::async_trait]
impl Fairing for Ratelimit {
	fn info(&self) -> Info {
		Info {
			name: "Ratelimit",
			kind: Kind::Request | Kind::Response | Kind::Liftoff,
		}
	}

	async fn on_liftoff(&self, _: &Rocket<Orbit>) {
		// Periodically decrement ratelimits
		let request_counts = self.request_counts.clone();
		rocket::tokio::task::spawn(async move {
			// The number of times per minute to reduce ratelimit counts
			const REDUCTION_RATE: u16 = 4;
			loop {
				rocket::tokio::time::sleep(Duration::from_secs((60 / REDUCTION_RATE) as u64)).await;
				for count in request_counts.lock().await.values_mut() {
					// One is added so that integer division imprecision doesn't make reduction take an extra cycle
					if *count >= RATELIMIT {
						*count -= RATELIMIT / REDUCTION_RATE + 1;
					} else {
						*count = 0;
					}
				}
			}
		});
	}

	async fn on_request(&self, request: &mut Request<'_>, _: &mut Data<'_>) {
		if let Some(ip) = request.client_ip() {
			*self.request_counts.lock().await.entry(ip).or_default() += 1;
		}
	}

	async fn on_response<'r>(&self, request: &'r Request<'_>, response: &mut Response<'r>) {
		if let Some(ip) = request.client_ip() {
			if let Some(count) = self.request_counts.lock().await.get(&ip) {
				if *count > RATELIMIT {
					response.set_header(ContentType::Text);
					response.set_status(Status::TooManyRequests);
					response.set_streamed_body(std::io::Cursor::new("Too many requests"));
					error!("Client {} made too many requests", ip);
				}
			}
		} else {
			response.set_header(ContentType::Text);
			response.set_status(Status::Forbidden);
			response.set_streamed_body(std::io::Cursor::new("IP address is missing in request"));
			error!("Client did not have an IP address");
		}
	}
}

/// Simple responder to set CORS allow header
pub struct CORSAllow<R>(pub R);

impl<'r, 'o: 'r, R: ResponderTrait<'r, 'o>> ResponderTrait<'r, 'o> for CORSAllow<R> {
	fn respond_to(self, request: &'r rocket::Request<'_>) -> rocket::response::Result<'o> {
		let mut out = self.0.respond_to(request);
		if let Ok(out) = &mut out {
			out.set_header(Header::new("Access-Control-Allow-Origin", "*"));
		}

		out
	}
}
