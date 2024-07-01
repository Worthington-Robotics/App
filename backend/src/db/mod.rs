use crate::member::Member;

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
}
