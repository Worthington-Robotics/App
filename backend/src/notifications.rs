use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Subscription {
	pub id: String,
	pub member: String,
	pub endpoint: String,
	pub p256dh_key: String,
	pub auth_key: String,
}
