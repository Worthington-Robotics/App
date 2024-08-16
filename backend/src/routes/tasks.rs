use rocket::form::{Form, FromForm};
use rocket::http::Status;
use tracing::{error, span, Level};

use crate::db::Database;
use crate::tasks::{Checklist, Task};
use crate::{routes::SessionID, State};

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
