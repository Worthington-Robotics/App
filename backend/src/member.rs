use std::{collections::HashSet, convert::Infallible, fmt::Display, str::FromStr};

use chrono::Utc;
use rocket::FromFormField;
use serde::{Deserialize, Serialize};
use strum_macros::EnumIter;

use crate::{auth::Privilege, util::ToDropdown};

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
	/// The date when this member was created
	#[serde(default = "default_creation_date")]
	pub creation_date: String,
}

impl Member {
	/// Check if this member has elevated permissions
	pub fn is_elevated(&self) -> bool {
		if self.id == "admin" {
			true
		} else {
			self.kind.get_privilege() == Privilege::Elevated
		}
	}
}

fn default_creation_date() -> String {
	Utc::now().to_rfc2822()
}

/// What kind of a member a member is
#[derive(Serialize, Deserialize, Clone, Copy, FromFormField, PartialEq, Eq, EnumIter)]
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

impl Display for MemberKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(
			f,
			"{}",
			match self {
				Self::Standard => "Standard",
				Self::Admin => "Admin",
			}
		)
	}
}

impl ToDropdown for MemberKind {
	fn to_dropdown(&self) -> &'static str {
		match self {
			Self::Standard => "Standard",
			Self::Admin => "Admin",
		}
	}
}

/// Different member groups
#[derive(
	Serialize,
	Deserialize,
	Clone,
	Copy,
	FromFormField,
	PartialEq,
	Eq,
	Hash,
	PartialOrd,
	Ord,
	EnumIter,
)]
#[serde(rename_all = "snake_case")]
pub enum MemberGroup {
	Member,
	NewMember,
	ReturningMember,
	PitCrew,
	DriveTeam,
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
				Self::ReturningMember => "Returning Member",
				Self::PitCrew => "Pit Crew",
				Self::DriveTeam => "Drive Team",
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
			Self::ReturningMember => "Returning Members",
			Self::PitCrew => "Pit Crew",
			Self::DriveTeam => "Drive Team",
			Self::Lead => "Leads",
			Self::President => "Presidents",
			Self::Coach => "Coaches",
			Self::Mentor => "Mentors",
		}
	}
}

impl ToDropdown for MemberGroup {
	fn to_dropdown(&self) -> &'static str {
		match self {
			Self::Member => "Member",
			Self::NewMember => "NewMember",
			Self::ReturningMember => "ReturningMember",
			Self::PitCrew => "PitCrew",
			Self::DriveTeam => "DriveTeam",
			Self::Lead => "Lead",
			Self::President => "President",
			Self::Coach => "Coach",
			Self::Mentor => "Mentor",
		}
	}
}

impl FromStr for MemberGroup {
	type Err = ();
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		match s {
			"Member" => Ok(Self::Member),
			"New Member" => Ok(Self::NewMember),
			"Returning Member" => Ok(Self::ReturningMember),
			"Pit Crew" => Ok(Self::PitCrew),
			"Drive Team" => Ok(Self::PitCrew),
			"Lead" => Ok(Self::Lead),
			"President" => Ok(Self::President),
			"Coach" => Ok(Self::Coach),
			"Mentor" => Ok(Self::Mentor),
			_ => Err(()),
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

/// A mention of a member or group of members
#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(rename_all = "snake_case")]
pub enum MemberMention {
	/// A single member ID
	Member(String),
	/// A group of members
	Group(MemberGroup),
}

impl MemberMention {
	/// Check if a member is mentioned by this
	pub fn mentions_member(&self, member: &Member) -> bool {
		match self {
			Self::Member(check) => check == &member.id,
			Self::Group(group) => member.groups.contains(group),
		}
	}

	/// Write this mention to a string for database usage
	pub fn to_db(&self) -> String {
		match self {
			Self::Member(member) => member.clone(),
			Self::Group(group) => format!("@{}", group.to_dropdown()),
		}
	}
}

impl FromStr for MemberMention {
	type Err = Infallible;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(match s {
			"@Member" => Self::Group(MemberGroup::Member),
			"@New Member" => Self::Group(MemberGroup::NewMember),
			"@Pit Crew" => Self::Group(MemberGroup::PitCrew),
			"@Lead" => Self::Group(MemberGroup::Lead),
			"@President" => Self::Group(MemberGroup::President),
			"@Coach" => Self::Group(MemberGroup::Coach),
			"@Mentor" => Self::Group(MemberGroup::Mentor),
			_ => Self::Member(s.to_string()),
		})
	}
}
