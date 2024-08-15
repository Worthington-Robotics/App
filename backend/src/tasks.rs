use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Checklist {
	pub id: String,
	pub name: String,
	/// List of task IDs
	pub tasks: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Task {
	pub id: String,
	pub checklist: String,
	pub text: String,
	pub done: bool,
}
