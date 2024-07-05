use std::{collections::HashSet, fmt::Display};

use rocket::FromFormField;
use serde::{Deserialize, Serialize};

use crate::auth::Privilege;

/// A member stored in the database and code
#[derive(Serialize, Deserialize, Clone)]
pub struct Member {
	/// The unique ID of this member
	pub id: String,
	/// This member's full name
	pub name: String,
	/// The kind of this member
	pub kind: MemberKind,
	/// The groups of this member
	#[serde(default)]
	pub groups: HashSet<MemberGroup>,
	/// This member's password, likely to be hashed
	pub password: String,
	/// This member's password salt
	pub password_salt: Option<String>,
}

/// What kind of a member a member is
#[derive(Serialize, Deserialize, Clone, Copy, FromFormField)]
#[serde(rename_all = "snake_case")]
pub enum MemberKind {
	Standard,
	Admin,
}

impl MemberKind {
	/// Get the permissions of this member kind
	pub fn get_privilege(&self) -> Privilege {
		match self {
			Self::Standard => Privilege::Standard,
			Self::Admin => Privilege::Elevated,
		}
	}
}

/// Different member groups
#[derive(Serialize, Deserialize, Clone, Copy, FromFormField, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum MemberGroup {
	Member,
	NewMember,
	PitCrew,
	Lead,
	President,
	Coach,
	Mentor,
}

impl Display for MemberGroup {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::Member => "Member",
				Self::NewMember => "New Member",
				Self::PitCrew => "Pit Crew",
				Self::Lead => "Lead",
				Self::President => "President",
				Self::Coach => "Coach",
				Self::Mentor => "Mentor",
			}
		)
	}
}

impl MemberGroup {
	pub fn to_plural_string(&self) -> &'static str {
		match self {
			Self::Member => "Members",
			Self::NewMember => "New Members",
			Self::PitCrew => "Pit Crew",
			Self::Lead => "Leads",
			Self::President => "Presidents",
			Self::Coach => "Coaches",
			Self::Mentor => "Mentors",
		}
	}
}

/// Count the number of members in a group
pub fn count_group_members<'a>(
	members: impl Iterator<Item = &'a Member>,
	group: &MemberGroup,
) -> usize {
	members
		.filter(|member| member.groups.contains(group))
		.count()
}
