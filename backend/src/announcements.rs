use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
	auth::Privilege,
	db::Database,
	member::{Member, MemberMention},
};

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
	/// Members who have read this announcement
	#[serde(default)]
	pub read: HashSet<String>,
}

impl Announcement {
	/// Checks if a member can see this announcement
	pub fn can_member_see(&self, member: &Member) -> bool {
		member.is_elevated()
			|| self.mentioned.iter().any(|x| x.mentions_member(&member))
	}
}

/// Count the number of unread announcements a member has
pub fn count_unread_announcements(member: &Member, db: &impl Database) -> usize {
	db.get_announcements()
		.filter(|x| x.can_member_see(member))
		.filter(|x| !x.read.contains(&member.id))
		.count()
}
