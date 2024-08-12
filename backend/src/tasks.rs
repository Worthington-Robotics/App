use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Checklist {
	pub id: String,
	pub name: String,
	/// List of task IDs
	pub tasks: Vec<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Task {
	pub id: String,
	pub done: bool,
}
