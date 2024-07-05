use crate::{events::Event, member::Member};

/// Simple JSON database
pub mod json;

/// Trait for the database that is used
pub trait Database {
	/// Open the database
	fn open() -> anyhow::Result<Self>
	where
		Self: Sized;

	/// Get a member by ID
	fn get_member(&self, id: &str) -> Option<Member>;

	/// Create a new member
	fn create_member(&mut self, member: Member) -> anyhow::Result<()>;

	/// Get all members
	fn get_members(&self) -> impl Iterator<Item = &Member>;

	/// Get an event by ID
	fn get_event(&self, event: &str) -> Option<Event>;

	/// Create a new event
	fn create_event(&mut self, event: Event) -> anyhow::Result<()>;

	/// Get all events
	fn get_events(&self) -> impl Iterator<Item = &Event>;
}
