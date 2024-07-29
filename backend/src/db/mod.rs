use crate::{
	announcements::Announcement, attendance::AttendanceEntry, events::Event, member::Member,
};

/// Simple JSON database
pub mod json;
/// Real SQL database
pub mod sql;

#[cfg(feature = "sqldb")]
pub type DatabaseImpl = sql::SqlDatabase;
#[cfg(not(feature = "sqldb"))]
pub type DatabaseImpl = json::JSONDatabase;

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
	async fn get_members(&self) -> impl Iterator<Item = Member>;

	/// Get an event by ID
	fn get_event(&self, event: &str) -> Option<Event>;

	/// Create a new event
	fn create_event(&mut self, event: Event) -> anyhow::Result<()>;

	/// Get all events
	fn get_events(&self) -> impl Iterator<Item = &Event>;

	/// Get an announcement by ID
	fn get_announcement(&self, announcement: &str) -> Option<Announcement>;

	/// Create a new announcement
	fn create_announcement(&mut self, announcement: Announcement) -> anyhow::Result<()>;

	/// Get all announcements
	fn get_announcements(&self) -> impl Iterator<Item = &Announcement>;

	/// Get all attendance records for a member
	fn get_attendance(&self, member: &str) -> Vec<AttendanceEntry>;

	/// Get the current attendance record for a member
	fn get_current_attendance(&self, member: &str) -> Option<AttendanceEntry>;

	/// Record attendance for a member
	fn record_attendance(&mut self, member: &str, event: &str) -> anyhow::Result<()>;

	/// Finish attending an event
	fn finish_attendance(&mut self, member: &str) -> anyhow::Result<()>;
}
