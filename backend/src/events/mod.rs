use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{
	auth::Privilege,
	member::{Member, MemberGroup},
};

/// A single event, stored in the database or code
#[derive(Serialize, Deserialize)]
pub struct Event {
	/// The unique ID for this event
	pub id: String,
	/// The display name for this event
	pub name: String,
	/// The date for this event
	pub date: String,
	/// The kind for this event
	#[serde(default)]
	pub kind: EventKind,
	/// The urgency for this event
	#[serde(default)]
	pub urgency: EventUrgency,
	/// The visibility for this event
	#[serde(default)]
	pub visibility: EventVisibility,
	/// Invites for this event
	#[serde(default)]
	pub invites: HashSet<EventInvite>,
	/// People attending the event, as a set of member IDs
	#[serde(default)]
	pub rsvp: HashSet<String>,
}

impl Event {
	/// Check if this event invites a user
	pub fn invites_member(&self, member: &Member) -> bool {
		for invite in &self.invites {
			let matches = match invite {
				EventInvite::Single(check) => check == &member.id,
				EventInvite::Group(group) => member.groups.contains(group),
			};
			if matches {
				return true;
			}
		}

		false
	}
}

/// Different kinds of events
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
	#[default]
	Meeting,
	Competition,
	Outreach,
}

/// Urgency for an event
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EventUrgency {
	#[default]
	Optional,
	Mandatory,
}

/// Visibility for an event
#[derive(Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum EventVisibility {
	#[default]
	Everyone,
	InviteOnly,
}

/// Invites for an event
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EventInvite {
	/// A single member ID
	Single(String),
	/// A group of members
	Group(MemberGroup),
}

/// Get all of the events relevant to a given member
pub fn get_relevant_events<'a>(
	member: &Member,
	events: impl Iterator<Item = &'a Event>,
) -> Vec<&'a Event> {
	let is_elevated = member.kind.get_privilege() == Privilege::Elevated;
	let events = events.filter(|event| {
		// If the member is not an admin which can see every event, hide invite-only events that this member is not invited to
		if !is_elevated
			&& event.visibility == EventVisibility::InviteOnly
			&& !event.invites_member(member)
		{
			return false;
		}

		true
	});

	events.collect()
}
