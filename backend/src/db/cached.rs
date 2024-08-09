use std::{sync::Arc, time::Duration};

use anyhow::Context;
use rocket::{
	fairing::{Fairing, Info, Kind},
	tokio::{sync::Mutex, try_join},
	Orbit, Rocket,
};
use tracing::{error, info};

use super::{json::JSONDatabase, sql::SqlDatabase, Database};

pub struct CacheDatabase {
	sql: SqlDatabase,
	cache: JSONDatabase,
}

impl CacheDatabase {
	/// Sync the cache with the remote database. Should be done periodically to
	/// provide protection against issues
	pub async fn sync_cache(&mut self) -> anyhow::Result<()> {
		self.cache = populate_cache(&self.sql).await?;

		Ok(())
	}
}

/// Populate the JSON cache with the SQL database's data
async fn populate_cache(sql: &SqlDatabase) -> anyhow::Result<JSONDatabase> {
	let mut cache = JSONDatabase::new(false).context("Failed to open cache database")?;

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

	Ok(cache)
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

		let cache = populate_cache(&sql)
			.await
			.context("Failed to populate cache")?;

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

/// Fairing for periodically syncing the cache
pub struct SyncCache {
	db: Arc<Mutex<CacheDatabase>>,
}

impl SyncCache {
	#[cfg(feature = "cachedb")]
	pub fn new(db: Arc<Mutex<CacheDatabase>>) -> Self {
		Self { db }
	}
}

#[async_trait::async_trait]
impl Fairing for SyncCache {
	fn info(&self) -> Info {
		Info {
			name: "Sync Cache",
			kind: Kind::Liftoff,
		}
	}

	async fn on_liftoff(&self, _: &Rocket<Orbit>) {
		// Periodically sync the cache
		let db = self.db.clone();
		rocket::tokio::spawn(async move {
			loop {
				rocket::tokio::time::sleep(Duration::from_secs(120)).await;
				info!("Syncing cache...");
				if let Err(e) = db.lock().await.sync_cache().await {
					error!("Failed to sync cache: {e}");
				}
			}
		});
	}
}
