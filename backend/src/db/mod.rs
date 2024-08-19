use crate::{
	announcements::Announcement,
	attendance::AttendanceEntry,
	events::Event,
	member::Member,
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
}
