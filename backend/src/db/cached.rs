use anyhow::Context;
use rocket::tokio::try_join;

use super::{json::JSONDatabase, sql::SqlDatabase, Database};

pub struct CacheDatabase {
	sql: SqlDatabase,
	cache: JSONDatabase,
}

impl Database for CacheDatabase {
	async fn open() -> anyhow::Result<Self>
	where
		Self: Sized,
	{
		// Open the databases
		let sql = SqlDatabase::open()
			.await
			.context("Failed to open SQL database")?;
		let mut cache = JSONDatabase::new(false).context("Failed to open cache database")?;

		// Populate the cache
		for member in sql
			.get_members()
			.await
			.context("Failed to get members from database")?
		{
			cache.create_member(member).await?;
		}
		for event in sql
			.get_events()
			.await
			.context("Failed to get events from database")?
		{
			cache.create_event(event).await?;
		}

		Ok(Self { sql, cache })
	}

	async fn get_member(&self, id: &str) -> anyhow::Result<Option<crate::member::Member>> {
		if let Some(member) = self.cache.get_member(id).await? {
			Ok(Some(member))
		} else {
			self.sql.get_member(id).await
		}
	}

	async fn create_member(&mut self, member: crate::member::Member) -> anyhow::Result<()> {
		try_join!(
			self.sql.create_member(member.clone()),
			self.cache.create_member(member)
		)?;

		Ok(())
	}

	async fn delete_member(&mut self, member: &str) -> anyhow::Result<()> {
		try_join!(
			self.sql.delete_member(member),
			self.cache.delete_member(member)
		)?;

		Ok(())
	}

	async fn get_members(&self) -> anyhow::Result<impl Iterator<Item = crate::member::Member>> {
		self.cache.get_members().await
	}

	async fn member_exists(&self, member: &str) -> anyhow::Result<bool> {
		self.cache.member_exists(member).await
	}

	async fn get_event(&self, event: &str) -> anyhow::Result<Option<crate::events::Event>> {
		if let Some(event) = self.cache.get_event(event).await? {
			Ok(Some(event))
		} else {
			self.sql.get_event(event).await
		}
	}

	async fn create_event(&mut self, event: crate::events::Event) -> anyhow::Result<()> {
		try_join!(
			self.sql.create_event(event.clone()),
			self.cache.create_event(event)
		)?;

		Ok(())
	}

	async fn delete_event(&mut self, event: &str) -> anyhow::Result<()> {
		try_join!(self.sql.delete_event(event), self.cache.delete_event(event))?;

		Ok(())
	}

	async fn get_events(&self) -> anyhow::Result<impl Iterator<Item = crate::events::Event>> {
		self.cache.get_events().await
	}

	async fn event_exists(&self, event: &str) -> anyhow::Result<bool> {
		self.cache.event_exists(event).await
	}

	fn get_announcement(&self, announcement: &str) -> Option<crate::announcements::Announcement> {
		self.sql.get_announcement(announcement)
	}

	fn create_announcement(
		&mut self,
		announcement: crate::announcements::Announcement,
	) -> anyhow::Result<()> {
		self.sql.create_announcement(announcement)
	}

	fn get_announcements(&self) -> impl Iterator<Item = &crate::announcements::Announcement> {
		self.sql.get_announcements()
	}

	fn get_attendance(&self, member: &str) -> Vec<crate::attendance::AttendanceEntry> {
		self.sql.get_attendance(member)
	}

	fn get_current_attendance(&self, member: &str) -> Option<crate::attendance::AttendanceEntry> {
		self.sql.get_current_attendance(member)
	}

	fn record_attendance(&mut self, member: &str, event: &str) -> anyhow::Result<()> {
		self.sql.record_attendance(member, event)
	}

	fn finish_attendance(&mut self, member: &str) -> anyhow::Result<()> {
		self.sql.finish_attendance(member)
	}
}
