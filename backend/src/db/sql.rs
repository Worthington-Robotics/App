#![allow(dead_code)]

use std::str::FromStr;

use anyhow::Context;
use sqlx::{
	postgres::{PgConnectOptions, PgPoolOptions},
	Executor, Pool, Postgres,
};

use crate::{
	announcements::Announcement, attendance::AttendanceEntry, events::Event, member::Member,
};

use super::Database;

pub struct SqlDatabase {
	pool: Pool<Postgres>,
}

impl Database for SqlDatabase {
	async fn open() -> anyhow::Result<Self>
	where
		Self: Sized,
	{
		let uri = std::env::var("DATABASE_URL")
			.context("Failed to get database URI from environment variable")?;
		let connect_options = PgConnectOptions::from_str(&uri)
			.context("Failed to parse connection URI")?
			.ssl_mode(sqlx::postgres::PgSslMode::Require);
		let pool = PgPoolOptions::new()
			.max_connections(1)
			.connect_with(connect_options)
			.await
			.context("Failed to open database connection")?;

		setup_database(&pool)
			.await
			.context("Failed to set up database")?;

		Ok(Self { pool })
	}

	fn get_member(&self, id: &str) -> Option<Member> {
		None
	}

	fn create_member(&mut self, member: Member) -> anyhow::Result<()> {
		Ok(())
	}

	fn delete_member(&mut self, member: &str) -> anyhow::Result<()> {
		Ok(())
	}

	fn get_members(&self) -> impl Iterator<Item = &Member> {
		std::iter::empty()
	}

	fn get_event(&self, event: &str) -> Option<Event> {
		None
	}

	fn create_event(&mut self, event: Event) -> anyhow::Result<()> {
		Ok(())
	}

	fn get_events(&self) -> impl Iterator<Item = &Event> {
		std::iter::empty()
	}

	fn get_announcement(&self, announcement: &str) -> Option<Announcement> {
		None
	}

	fn create_announcement(&mut self, announcement: Announcement) -> anyhow::Result<()> {
		Ok(())
	}

	fn get_announcements(&self) -> impl Iterator<Item = &Announcement> {
		std::iter::empty()
	}

	fn get_attendance(&self, member: &str) -> Vec<AttendanceEntry> {
		Vec::new()
	}

	fn get_current_attendance(&self, member: &str) -> Option<AttendanceEntry> {
		None
	}

	fn record_attendance(&mut self, member: &str, event: &str) -> anyhow::Result<()> {
		Ok(())
	}

	fn finish_attendance(&mut self, member: &str) -> anyhow::Result<()> {
		Ok(())
	}
}

/// Setup the database
async fn setup_database(pool: &Pool<Postgres>) -> anyhow::Result<()> {
	pool.execute("CREATE TABLE IF NOT EXISTS members (Id text, Name text, Kind text, Groups text[], Password text, PasswordSalt text, CreationDate text)")
		.await
		.context("Failed to setup members table")?;

	Ok(())
}
