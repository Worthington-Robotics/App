use std::{collections::HashSet, fmt::Display};

use rocket::FromFormField;
use serde::{Deserialize, Serialize};

use crate::auth::Privilege;
use crate::member::{Member, MemberMention};

/// A single event, stored in the database or code
#[derive(Serialize, Deserialize, Clone)]
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
	pub invites: HashSet<MemberMention>,
	/// People attending the event, as a set of member IDs
	#[serde(default)]
	pub rsvp: HashSet<String>,
}

impl Event {
	/// Check if this event invites a user
	pub fn invites_member(&self, member: &Member) -> bool {
		self.invites.iter().any(|x| x.mentions_member(member))
	}
}

/// Different kinds of events
#[derive(Serialize, Deserialize, Default, FromFormField, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
	#[default]
	Meeting,
	Competition,
	Outreach,
	Fundraising,
}

impl Display for EventKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::Meeting => "Meeting",
				Self::Competition => "Competition",
				Self::Outreach => "Outreach",
				Self::Fundraising => "Fundraising",
			}
		)
	}
}

/// Urgency for an event
#[derive(Serialize, Deserialize, Default, FromFormField, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum EventUrgency {
	#[default]
	Optional,
	Mandatory,
}

/// Visibility for an event
#[derive(Serialize, Deserialize, PartialEq, Eq, Default, FromFormField, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum EventVisibility {
	#[default]
	Everyone,
	InviteOnly,
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
