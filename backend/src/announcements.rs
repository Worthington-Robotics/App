use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::member::MemberMention;

#[derive(Serialize, Deserialize, Clone)]
pub struct Announcement {
	/// The unique ID of the announcement
	pub id: String,
	/// The title of the announcement
	pub title: String,
	/// The date when this announcement was posted
	pub date: String,
	/// The body of the announcement
	#[serde(default)]
	pub body: Option<String>,
	/// An event associated with this announcement
	#[serde(default)]
	pub event: Option<String>,
	/// Members mentioned in this announcement
	#[serde(default)]
	pub mentioned: HashSet<MemberMention>,
}
