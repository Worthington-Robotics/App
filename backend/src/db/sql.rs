#![allow(dead_code)]

use std::str::FromStr;

use anyhow::{anyhow, Context};
use rocket::futures::TryStreamExt;
use sqlx::{
	postgres::{PgConnectOptions, PgPoolOptions, PgRow},
	Executor, Pool, Postgres, Row,
};
use tracing::error;

use crate::{
	announcements::Announcement,
	attendance::AttendanceEntry,
	events::Event,
	member::{Member, MemberGroup, MemberKind},
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

	async fn get_member(&self, id: &str) -> anyhow::Result<Option<Member>> {
		let mut result = sqlx::query("SELECT * FROM members WHERE Id = $1")
			.bind(id)
			.fetch(&self.pool);
		let row = result.try_next().await;
		match row {
			Ok(row) => {
				let Some(row) = row else {
					return Ok(None);
				};
				let member = read_member(id, row).context("Failed to read member")?;

				Ok(Some(member))
			}
			Err(e) => {
				error!("Failed to get member {id} from database: {e}");
				Err(anyhow!("Failed to get member from database"))
			}
		}
	}

	async fn create_member(&mut self, member: Member) -> anyhow::Result<()> {
		// Remove the existing member
		self.delete_member(&member.id)
			.await
			.context("Failed to delete existing member")?;
		sqlx::query("INSERT INTO members (Id, Name, Kind, Groups, Password, PasswordSalt, CreationDate) VALUES ($1, $2, $3, $4, $5, $6, $7)")
			.bind(member.id)
			.bind(member.name)
			.bind(member.kind.to_string())
			.bind(
				member
					.groups
					.into_iter()
					.map(|x| x.to_string())
					.collect::<Vec<_>>(),
			)
			.bind(member.password)
			.bind(member.password_salt)
			.bind(member.creation_date)
			.execute(&self.pool)
			.await
			.context("Failed to create new member in database")?;

		Ok(())
	}

	async fn delete_member(&mut self, member: &str) -> anyhow::Result<()> {
		let query = sqlx::query("DELETE FROM members WHERE Id = $1").bind(member);

		query
			.execute(&self.pool)
			.await
			.context("Failed to remove member from database")?;

		Ok(())
	}

	async fn get_members(&self) -> anyhow::Result<impl Iterator<Item = Member>> {
		let result = sqlx::query("SELECT * FROM members")
			.fetch_all(&self.pool)
			.await;
		match result {
			Ok(rows) => {
				let mut out = Vec::with_capacity(rows.len());
				for row in rows {
					let id: String = row.try_get("id")?;
					let member = read_member(&id, row).context("Failed to read member")?;
					out.push(member);
				}

				Ok(out.into_iter())
			}
			Err(e) => {
				error!("Failed to get all members from database: {e}");
				Err(anyhow!("Failed to get members from database"))
			}
		}
	}

	async fn member_exists(&self, member: &str) -> anyhow::Result<bool> {
		let result = sqlx::query("SELECT 1 FROM members WHERE Id = $1")
			.bind(member)
			.execute(&self.pool)
			.await
			.context("Failed to query database for member")?;

		Ok(result.rows_affected() > 0)
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
	pool.execute("CREATE TABLE IF NOT EXISTS members (Id text PRIMARY KEY, Name text, Kind text, Groups text[], Password text, PasswordSalt text, CreationDate text)")
		.await
		.context("Failed to set up members table")?;

	Ok(())
}

/// Read a member from the database
fn read_member(id: &str, row: PgRow) -> anyhow::Result<Member> {
	let name: &str = row.try_get("name")?;
	let kind: &str = row.try_get("kind")?;
	let kind = match kind {
		"Standard" => MemberKind::Standard,
		"Admin" => MemberKind::Admin,
		other => {
			error!("Unknown member kind {other}");
			return Err(anyhow!("Unknown member kind"));
		}
	};
	let groups: Vec<String> = row.try_get("groups")?;
	let groups = groups
		.into_iter()
		.filter_map(|x| MemberGroup::from_str(&x).ok());
	let password: &str = row.try_get("password")?;
	let password_salt: Option<String> = row.try_get("passwordsalt")?;
	let creation_date: &str = row.try_get("creationdate")?;

	Ok(Member {
		id: id.to_string(),
		name: name.to_string(),
		kind,
		groups: groups.collect(),
		password: password.to_string(),
		password_salt,
		creation_date: creation_date.to_string(),
	})
}
