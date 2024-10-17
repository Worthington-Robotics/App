use serde::{Deserialize, Serialize};

use crate::{
	announcements::Announcement,
	attendance::AttendanceEntry,
	events::Event,
	member::Member,
	scouting::{
		assignment::{MatchClaims, ScoutingAssignment},
		autos::Auto,
		matches::{Match, MatchNumber, MatchStats},
		status::StatusUpdate,
		Competition, Division, Team, TeamInfo, TeamNumber,
	},
	tasks::{Checklist, Task},
};

/// Combination of the two databases
pub mod cached;
/// Simple JSON database
pub mod json;
/// Real SQL database
pub mod sql;

#[cfg(feature = "sqldb")]
pub type DatabaseImpl = sql::SqlDatabase;
#[cfg(not(feature = "sqldb"))]
#[cfg(not(feature = "cachedb"))]
pub type DatabaseImpl = json::JSONDatabase;
#[cfg(feature = "cachedb")]
pub type DatabaseImpl = cached::CacheDatabase;

/// Trait for the database that is used
pub trait Database {
	/// Open the database
	async fn open() -> anyhow::Result<Self>
	where
		Self: Sized;

	/// Get a member by ID
	async fn get_member(&self, id: &str) -> anyhow::Result<Option<Member>>;

	/// Create a new member
	async fn create_member(&mut self, member: Member) -> anyhow::Result<()>;

	/// Delete a member
	async fn delete_member(&mut self, member: &str) -> anyhow::Result<()>;

	/// Get all members
	async fn get_members(&self) -> anyhow::Result<impl Iterator<Item = Member>>;

	/// Check if a member exists
	async fn member_exists(&self, member: &str) -> anyhow::Result<bool>;

	/// Get an event by ID
	async fn get_event(&self, event: &str) -> anyhow::Result<Option<Event>>;

	/// Create a new event
	async fn create_event(&mut self, event: Event) -> anyhow::Result<()>;

	/// Delete an event
	async fn delete_event(&mut self, event: &str) -> anyhow::Result<()>;

	/// Get all events
	async fn get_events(&self) -> anyhow::Result<impl Iterator<Item = Event>>;

	/// Check if an event exists
	async fn event_exists(&self, event: &str) -> anyhow::Result<bool>;

	/// Get an announcement by ID
	async fn get_announcement(&self, announcement: &str) -> anyhow::Result<Option<Announcement>>;

	/// Create a new announcement
	async fn create_announcement(&mut self, announcement: Announcement) -> anyhow::Result<()>;

	/// Get all announcements
	async fn get_announcements(&self) -> anyhow::Result<impl Iterator<Item = Announcement>>;

	/// Mark an announcement as read
	async fn read_announcement(&mut self, announcement: &str, member: &str) -> anyhow::Result<()>;

	/// Delete an announcement
	async fn delete_announcement(&mut self, announcement: &str) -> anyhow::Result<()>;

	/// Get all attendance records for a member
	async fn get_attendance(&self, member: &str) -> anyhow::Result<Vec<AttendanceEntry>>;

	/// Get the current attendance record for a member
	async fn get_current_attendance(&self, member: &str)
		-> anyhow::Result<Option<AttendanceEntry>>;

	/// Record attendance for a member
	async fn record_attendance(&mut self, member: &str, event: &str) -> anyhow::Result<()>;

	/// Finish attending an event
	async fn finish_attendance(&mut self, member: &str) -> anyhow::Result<()>;

	/// Get a checklist
	async fn get_checklist(&self, checklist: &str) -> anyhow::Result<Option<Checklist>>;

	/// Create a checklist
	async fn create_checklist(&mut self, checklist: Checklist) -> anyhow::Result<()>;

	/// Delete a checklist
	async fn delete_checklist(&mut self, checklist: &str) -> anyhow::Result<()>;

	/// Get all checklists
	async fn get_checklists(&self) -> anyhow::Result<impl Iterator<Item = Checklist>>;

	/// Get a list of tasks from a checklist
	async fn get_checklist_tasks(
		&self,
		checklist: &str,
	) -> anyhow::Result<impl Iterator<Item = Task>>;

	/// Get a task
	async fn get_task(&self, task: &str) -> anyhow::Result<Option<Task>>;

	/// Create a task
	async fn create_task(&mut self, task: Task) -> anyhow::Result<()>;

	/// Do / undo a task
	async fn update_task(&mut self, task: &str) -> anyhow::Result<()>;

	/// Delete a task
	async fn delete_task(&mut self, task: &str) -> anyhow::Result<()>;

	/// Get a list of all tasks
	async fn get_tasks(&self) -> anyhow::Result<impl Iterator<Item = Task>>;

	/// Get the member from a calendar ID
	async fn get_calendar(&self, calendar_id: &str) -> anyhow::Result<Option<Member>>;

	/// Get a team
	async fn get_team(&self, team: TeamNumber) -> anyhow::Result<Option<Team>>;

	/// Create a team
	async fn create_team(&mut self, team: Team) -> anyhow::Result<()>;

	/// Delete a team
	async fn delete_team(&mut self, team: TeamNumber) -> anyhow::Result<()>;

	/// Get a list of all teams
	async fn get_teams(&self) -> anyhow::Result<impl Iterator<Item = Team>>;

	/// Create match stats
	async fn create_match_stats(&mut self, stats: MatchStats) -> anyhow::Result<()>;

	/// Get a list of all match stats
	async fn get_all_match_stats(&self) -> anyhow::Result<impl Iterator<Item = MatchStats>>;

	/// Get team info
	async fn get_team_info(&self, team: TeamNumber) -> anyhow::Result<Option<TeamInfo>>;

	/// Create team info for a team
	async fn create_team_info(&mut self, team: TeamNumber, info: TeamInfo) -> anyhow::Result<()>;

	/// Get an auto
	async fn get_auto(&self, auto: &str) -> anyhow::Result<Option<Auto>>;

	/// Create an auto
	async fn create_auto(&mut self, auto: Auto) -> anyhow::Result<()>;

	/// Delete an auto
	async fn delete_auto(&mut self, auto: &str) -> anyhow::Result<()>;

	/// Get a list of all autos from a team
	async fn get_autos(&self, team: TeamNumber) -> anyhow::Result<impl Iterator<Item = Auto>>;

	/// Get the list of status updates for a team
	async fn get_team_status(&self, team: TeamNumber) -> anyhow::Result<Vec<StatusUpdate>>;

	/// Add a status update to a team
	async fn update_team_status(&mut self, update: StatusUpdate) -> anyhow::Result<()>;

	/// Get the list of all status updates
	async fn get_all_status(&self) -> anyhow::Result<Vec<StatusUpdate>>;

	/// Get the match schedule
	async fn get_matches(&self) -> anyhow::Result<impl Iterator<Item = Match>>;

	/// Add a match to the schedule
	async fn create_match(&mut self, m: Match) -> anyhow::Result<()>;

	/// Remove all matches from the schedule
	async fn clear_matches(&mut self) -> anyhow::Result<()>;

	/// Get the prescouting assignment for a member
	async fn get_prescouting_assignment(
		&self,
		member: &str,
	) -> anyhow::Result<Option<ScoutingAssignment>>;

	/// Get all prescouting assignments
	async fn get_all_prescouting_assignments(
		&self,
	) -> anyhow::Result<impl Iterator<Item = ScoutingAssignment>>;

	/// Assign prescouting to a member
	async fn create_prescouting_assignment(
		&mut self,
		assignment: ScoutingAssignment,
	) -> anyhow::Result<()>;

	/// Get the claims for a match
	async fn get_match_claims(&self, m: &MatchNumber) -> anyhow::Result<Option<MatchClaims>>;

	/// Get claims for all matches
	async fn get_all_match_claims(&self) -> anyhow::Result<impl Iterator<Item = MatchClaims>>;

	/// Create claims for a match
	async fn create_match_claims(&mut self, claims: MatchClaims) -> anyhow::Result<()>;

	/// Clear all match claims
	async fn clear_match_claims(&mut self) -> anyhow::Result<()>;

	/// Get global data
	async fn get_global_data(&self) -> anyhow::Result<GlobalData>;

	/// Set global data
	async fn set_global_data(&mut self, data: GlobalData) -> anyhow::Result<()>;
}

/// Global data struct, for persistent values like global configuration
#[derive(Serialize, Deserialize, Default, Clone)]
#[serde(default)]
pub struct GlobalData {
	pub current_competition: Option<Competition>,
	pub current_division: Option<Division>,
}
