use std::str::FromStr;
use std::{collections::HashSet, fmt::Display};

use anyhow::Context;
use chrono::{DateTime, Datelike, Duration, FixedOffset, Utc};
use rocket::FromFormField;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;
use tracing::error;

use crate::member::{Member, MemberMention};
use crate::util::ToDropdown;

/// A single event, stored in the database or code
#[derive(Serialize, Deserialize, Clone)]
pub struct Event {
	/// The unique ID for this event
	pub id: String,
	/// The display name for this event
	pub name: String,
	/// The description for this event
	#[serde(default)]
	pub description: String,
	/// The date for this event
	pub date: String,
	/// The end date for this event
	#[serde(default)]
	pub end_date: Option<String>,
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
	/// People not attending the event, as a set of member IDs
	#[serde(default)]
	pub rsvp_no: HashSet<String>,
}

impl Event {
	/// Check if this event invites a member
	pub fn invites_member(&self, member: &Member) -> bool {
		self.invites.iter().any(|x| x.mentions_member(member))
	}

	/// Get the end date of this event, or it's heuristic end date if it has none
	pub fn get_end_date(&self) -> anyhow::Result<DateTime<FixedOffset>> {
		if let Some(end_date) = &self.end_date {
			DateTime::parse_from_rfc2822(end_date).context("Failed to parse date")
		} else {
			Ok(
				DateTime::parse_from_rfc2822(&self.date).context("Failed to parse date")?
					+ Duration::hours(EXPIRED_EVENT_THRESHOLD),
			)
		}
	}

	/// Check if this event is upcoming
	pub fn is_upcoming(&self, now: &DateTime<Utc>) -> bool {
		let Ok(end_date) = self.get_end_date() else {
			error!("Failed to parse end date for event");
			return true;
		};

		let end_date = end_date.with_timezone(&Utc);
		let diff = now.timestamp() - end_date.timestamp();
		if diff > EXPIRED_EVENT_THRESHOLD * 3600 {
			false
		} else {
			true
		}
	}

	/// Get the RSVP status of the specified member
	pub fn get_rsvp(&self, member: &str) -> RSVPStatus {
		if !self.rsvp.contains(member) {
			if !self.rsvp_no.contains(member) {
				RSVPStatus::Unknown
			} else {
				RSVPStatus::NotGoing
			}
		} else {
			RSVPStatus::Going
		}
	}
}

/// Different kinds of events
#[derive(Serialize, Deserialize, Default, FromFormField, Clone, Copy, EnumIter, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
	#[default]
	Meeting,
	Competition,
	Outreach,
	Fundraising,
}

impl ToDropdown for EventKind {
	fn to_dropdown(&self) -> &'static str {
		match self {
			Self::Meeting => "Meeting",
			Self::Competition => "Competition",
			Self::Outreach => "Outreach",
			Self::Fundraising => "Fundraising",
		}
	}
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

impl FromStr for EventKind {
	type Err = ();
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"Meeting" => Ok(Self::Meeting),
			"Competition" => Ok(Self::Competition),
			"Outreach" => Ok(Self::Outreach),
			"Fundraising" => Ok(Self::Fundraising),
			_ => Err(()),
		}
	}
}

/// Urgency for an event
#[derive(Serialize, Deserialize, Default, FromFormField, Clone, Copy, EnumIter, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EventUrgency {
	#[default]
	Optional,
	Mandatory,
}

impl FromStr for EventUrgency {
	type Err = ();
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"Optional" => Ok(Self::Optional),
			"Mandatory" => Ok(Self::Mandatory),
			_ => Err(()),
		}
	}
}

impl ToDropdown for EventUrgency {
	fn to_dropdown(&self) -> &'static str {
		match self {
			Self::Optional => "Optional",
			Self::Mandatory => "Mandatory",
		}
	}
}

impl Display for EventUrgency {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::Optional => "Optional",
				Self::Mandatory => "Mandatory",
			}
		)
	}
}

/// Visibility for an event
#[derive(Serialize, Deserialize, PartialEq, Eq, Default, FromFormField, Clone, Copy, EnumIter)]
#[serde(rename_all = "snake_case")]
pub enum EventVisibility {
	#[default]
	Everyone,
	InviteOnly,
}

impl ToDropdown for EventVisibility {
	fn to_dropdown(&self) -> &'static str {
		match self {
			Self::Everyone => "Everyone",
			Self::InviteOnly => "InviteOnly",
		}
	}
}

impl FromStr for EventVisibility {
	type Err = ();
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"Everyone" => Ok(Self::Everyone),
			"InviteOnly" => Ok(Self::InviteOnly),
			_ => Err(()),
		}
	}
}

impl Display for EventVisibility {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::Everyone => "Everyone",
				Self::InviteOnly => "Invite-Only",
			}
		)
	}
}

/// RSVP status for a member and event
#[derive(Serialize, Deserialize, Default, FromFormField, Clone, Copy, EnumIter, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RSVPStatus {
	#[default]
	Unknown,
	Going,
	NotGoing,
}

impl FromStr for RSVPStatus {
	type Err = ();
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"Unknown" => Ok(Self::Unknown),
			"Going" => Ok(Self::Going),
			"NotGoing" => Ok(Self::NotGoing),
			_ => Err(()),
		}
	}
}

impl ToDropdown for RSVPStatus {
	fn to_dropdown(&self) -> &'static str {
		match self {
			Self::Unknown => "Unknown",
			Self::Going => "Going",
			Self::NotGoing => "NotGoing",
		}
	}
}

impl Display for RSVPStatus {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::Unknown => "Unknown",
				Self::Going => "Going",
				Self::NotGoing => "Not Going",
			}
		)
	}
}

/// Get all of the events relevant to a given member
pub fn get_relevant_events<'a>(
	member: &Member,
	events: impl Iterator<Item = &'a Event>,
) -> Vec<&'a Event> {
	let is_elevated = member.is_elevated();
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

/// Threshold for how long ago events without end dates can be before they are not considered upcoming, in hours
pub const EXPIRED_EVENT_THRESHOLD: i64 = 1;

/// Get all of the events that are upcoming
pub fn get_upcoming_events<'a>(events: Vec<&'a Event>) -> Vec<&'a Event> {
	if events.is_empty() {
		return events;
	}

	let now = Utc::now();
	events
		.into_iter()
		.filter(|event| event.is_upcoming(&now))
		.collect()
}

/// Get the competition season of a date
pub fn get_season(date: &DateTime<Utc>) -> u32 {
	// Allow overriding with an environment variable
	if let Ok(season) = std::env::var("FRC_SEASON") {
		if let Ok(season) = season.parse() {
			return season;
		}
	}

	// Pre-season
	if date.month() >= 9 {
		date.year() as u32 + 1
	} else {
		date.year() as u32
	}
}

/// Format minutes as hours, minutes
pub fn format_minutes(minutes: u32) -> String {
	let hours = minutes / 60;
	let minutes = minutes % 60;

	let hours = if hours > 0 {
		format!("{hours} hours, ")
	} else {
		String::new()
	};

	format!("{hours}{minutes} minutes")
}
