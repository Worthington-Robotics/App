use itertools::Itertools;
use rocket::form::{Form, FromForm};
use rocket::http::Status;
use rocket::response::content::RawHtml;
use rocket::response::Redirect;
use tracing::{error, span, Level};

use crate::db::Database;
use crate::routes::OptionalSessionID;
use crate::tasks::{Checklist, Task};
use crate::{routes::SessionID, State};

use super::{create_page, PageOrRedirect};

#[rocket::get("/checklists")]
pub async fn checklists(
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Checklists");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	let Ok(requesting_member) = session_id.get_requesting_member(state).await else {
		return Ok(redirect);
	};

	let page = include_str!("pages/tasks/checklists.min.html");

	let lock = state.db.lock().await;
	let checklists = lock
		.get_checklists()
		.await
		.map_err(|e| {
			error!("Failed to get checklists from database: {e}");
			Status::InternalServerError
		})?
		.sorted_by_key(|x| x.tasks.len())
		.rev();
	let mut checklists_string = String::new();

	for checklist in checklists {
		checklists_string.push_str(&render_checklist(checklist));
	}
	let page = page.replace("{{checklists}}", &checklists_string);

	let add_button = if requesting_member.is_elevated() {
		format!(
			"<a href=\"/create_checklist\">{}</a>",
			include_str!("components/ui/new.min.html")
		)
	} else {
		String::new()
	};

	let page = page.replace("{{add-checklist}}", &add_button);

	let page = create_page("Inbox", &page);

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

fn render_checklist(checklist: Checklist) -> String {
	let component = include_str!("components/checklist.min.html");
	let out = component.replace("{{name}}", &checklist.name);
	let out = out.replace("{{progress}}", &checklist.tasks.len().to_string());

	out
}

#[rocket::post("/api/create_checklist", data = "<checklist>")]
pub async fn create_checklist(
	state: &State,
	session_id: SessionID<'_>,
	checklist: Form<ChecklistForm>,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Creating checklist");
	let _enter = span.enter();

	session_id.get_requesting_member(state).await?;

	let mut lock = state.db.lock().await;

	let checklist = Checklist {
		id: checklist.id.clone(),
		name: checklist.name.clone(),
		tasks: Vec::new(),
	};

	if let Err(e) = lock.create_checklist(checklist).await {
		error!("Failed to create checklist in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

#[derive(FromForm)]
pub struct ChecklistForm {
	id: String,
	name: String,
}

#[rocket::post("/api/create_task", data = "<task>")]
pub async fn create_task(
	state: &State,
	session_id: SessionID<'_>,
	task: Form<TaskForm>,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Creating task");
	let _enter = span.enter();

	session_id.get_requesting_member(state).await?;

	let mut lock = state.db.lock().await;

	let task = Task {
		id: task.id.clone(),
		checklist: task.checklist.clone(),
		text: task.text.clone(),
		done: false,
	};

	if let Err(e) = lock.create_task(task).await {
		error!("Failed to create task in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

#[derive(FromForm)]
pub struct TaskForm {
	id: String,
	checklist: String,
	text: String,
}

#[rocket::delete("/api/delete_checklist/<id>")]
pub async fn delete_checklist(
	state: &State,
	session_id: SessionID<'_>,
	id: &str,
) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Deleting checklist");
	let _enter = span.enter();

	session_id.get_requesting_member(state).await?;

	let mut lock = state.db.lock().await;

	if let Err(e) = lock.delete_checklist(id).await {
		error!("Failed to delete checklist {id} in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

#[rocket::delete("/api/delete_task/<id>")]
pub async fn delete_task(state: &State, session_id: SessionID<'_>, id: &str) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Deleting task");
	let _enter = span.enter();

	session_id.get_requesting_member(state).await?;

	let mut lock = state.db.lock().await;

	if let Err(e) = lock.delete_task(id).await {
		error!("Failed to delete task {id} in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}

#[rocket::post("/api/update_task/<id>")]
pub async fn update_task(state: &State, session_id: SessionID<'_>, id: &str) -> Result<(), Status> {
	let span = span!(Level::DEBUG, "Updating task");
	let _enter = span.enter();

	session_id.get_requesting_member(state).await?;

	let mut lock = state.db.lock().await;

	if let Err(e) = lock.update_task(id).await {
		error!("Failed to update task {id} in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(())
}
