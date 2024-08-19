use std::collections::HashMap;

use itertools::Itertools;
use rocket::form::{Form, FromForm};
use rocket::http::Status;
use rocket::response::content::RawHtml;
use rocket::response::Redirect;
use tracing::{error, span, Level};

use crate::db::Database;
use crate::routes::OptionalSessionID;
use crate::tasks::{Checklist, Task};
use crate::util::generate_id;
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

	let page = create_page("Todo", &page);

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

fn render_checklist(checklist: Checklist) -> String {
	let out = include_str!("components/tasks/checklist.min.html");
	let out = out.replace("{{id}}", &checklist.id);
	let out = out.replace("{{name}}", &checklist.name);
	let out = out.replace("{{progress}}", &format!("{} tasks", checklist.tasks.len()));

	out
}

#[rocket::get("/create_checklist?<id>")]
pub async fn create_checklist_page(
	id: Option<&str>,
	session_id: OptionalSessionID<'_>,
	state: &State,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Create checklist page");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	if session_id.verify_elevated(state).await.is_err() {
		return Ok(redirect);
	};

	let checklist = if let Some(id) = id {
		let lock = state.db.lock().await;
		// We are editing an existing checklist
		lock.get_checklist(id)
			.await
			.map_err(|e| {
				error!("Failed to get checklist from database: {e}");
				Status::InternalServerError
			})?
			.ok_or_else(|| {
				error!("Checklist does not exist: {}", id);
				Status::BadRequest
			})?
	} else {
		// We are making a new checklist
		Checklist {
			id: generate_id(),
			name: String::new(),
			tasks: Vec::new(),
		}
	};

	let page = include_str!("pages/tasks/create_checklist.min.html");
	let page = page.replace("{{id}}", &checklist.id);
	let page = page.replace("{{name}}", &checklist.name);
	let page = create_page("Create Checklist", &page);

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

#[rocket::get("/checklist/<id>")]
pub async fn checklist_page(
	session_id: OptionalSessionID<'_>,
	state: &State,
	id: &str,
) -> Result<PageOrRedirect, Status> {
	let span = span!(Level::DEBUG, "Checklist page");
	let _enter = span.enter();

	let redirect = PageOrRedirect::Redirect(Redirect::to("/login"));
	let Some(session_id) = session_id.to_session_id() else {
		return Ok(redirect);
	};

	let Ok(requesting_member) = session_id.get_requesting_member(state).await else {
		return Ok(redirect);
	};

	let lock = state.db.lock().await;

	let checklist = lock
		.get_checklist(id)
		.await
		.map_err(|e| {
			error!("Failed to get checklist from database: {e}");
			Status::InternalServerError
		})?
		.ok_or_else(|| {
			error!("Checklist does not exist: {}", id);
			Status::BadRequest
		})?;

	let tasks: HashMap<_, _> = lock
		.get_checklist_tasks(id)
		.await
		.map_err(|e| {
			error!("Failed to get checklist tasks from database: {e}");
			Status::InternalServerError
		})?
		.map(|x| (x.id.clone(), x))
		.collect();

	let page = include_str!("pages/tasks/checklist.min.html");

	let mut tasks_string = String::new();
	for task in &checklist.tasks {
		let Some(task) = tasks.get(task) else {
			continue;
		};
		let task = render_task(task);
		tasks_string.push_str(&task);
	}
	let page = page.replace("{{tasks}}", &tasks_string);

	let page = page.replace("{{id}}", &checklist.id);
	let page = page.replace("{{name}}", &checklist.name);
	let page = page.replace(
		"{{edit}}",
		if requesting_member.is_elevated() {
			include_str!("components/ui/edit.min.html")
		} else {
			""
		},
	);
	let page = page.replace(
		"{{delete}}",
		if requesting_member.is_elevated() {
			include_str!("components/ui/delete.min.html")
		} else {
			""
		},
	);

	let page = create_page("Checklist", &page);

	Ok(PageOrRedirect::Page(RawHtml(page)))
}

fn render_task(task: &Task) -> String {
	let out = include_str!("components/tasks/task.min.html");
	let out = out.replace("{{id}}", &task.id);
	let out = out.replace("{{text}}", &task.text);
	let out = out.replace("{{checked}}", if task.done { " checked" } else { "" });
	let out = out.replace("{{delete}}", include_str!("components/ui/delete.min.html"));

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

	session_id.verify_elevated(state).await?;

	let mut lock = state.db.lock().await;

	let existing_checklist = lock.get_checklist(&checklist.id).await.map_err(|e| {
		error!("Failed to get checklist from database: {e}");
		Status::InternalServerError
	})?;

	let checklist = Checklist {
		id: checklist.id.clone(),
		name: checklist.name.clone(),
		// Don't overwrite existing tasks
		tasks: existing_checklist.map(|x| x.tasks).unwrap_or_default(),
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
) -> Result<String, Status> {
	let span = span!(Level::DEBUG, "Creating task");
	let _enter = span.enter();

	session_id.get_requesting_member(state).await?;

	let mut lock = state.db.lock().await;

	// Add the task to the checklist
	let Some(mut checklist) = lock.get_checklist(&task.checklist).await.map_err(|e| {
		error!("Failed to get checklist from database: {e}");
		Status::InternalServerError
	})?
	else {
		error!("Checklist does not exist");
		return Err(Status::BadRequest);
	};

	let id = generate_id();

	if checklist.tasks.contains(&id) {
		error!("Attempted to add already existing task to checklist");
		return Err(Status::BadRequest);
	}

	checklist.tasks.push(id.clone());
	if let Err(e) = lock.create_checklist(checklist).await {
		error!("Failed to update checklist in database: {e}");
		return Err(Status::InternalServerError);
	}

	let task = Task {
		id: id.clone(),
		checklist: task.checklist.clone(),
		text: task.text.clone(),
		done: false,
	};

	if let Err(e) = lock.create_task(task).await {
		error!("Failed to create task in database: {e}");
		return Err(Status::InternalServerError);
	}

	Ok(id)
}

#[derive(FromForm)]
pub struct TaskForm {
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

	let Some(task) = lock.get_task(id).await.map_err(|e| {
		error!("Failed to get existing task from database: {e}");
		Status::InternalServerError
	})?
	else {
		error!("Task does not exist");
		return Err(Status::NotFound);
	};

	if let Err(e) = lock.delete_task(id).await {
		error!("Failed to delete task {id} in database: {e}");
		return Err(Status::InternalServerError);
	}

	// Remove the task from the checklist
	let Some(mut checklist) = lock.get_checklist(&task.checklist).await.map_err(|e| {
		error!("Failed to get checklist from database: {e}");
		Status::InternalServerError
	})?
	else {
		error!("Checklist does not exist");
		return Err(Status::BadRequest);
	};

	if let Some(pos) = checklist.tasks.iter().position(|x| *x == id) {
		checklist.tasks.remove(pos);
	}
	if let Err(e) = lock.create_checklist(checklist).await {
		error!("Failed to update checklist in database: {e}");
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
