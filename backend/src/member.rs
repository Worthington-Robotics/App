use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Member {
	pub id: String,
	pub name: String,
}
