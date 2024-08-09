#![allow(dead_code)]

use std::str::FromStr;

use anyhow::{anyhow, Context};
use chrono::Utc;
use rocket::futures::TryStreamExt;
use sqlx::{
	postgres::{PgConnectOptions, PgPoolOptions, PgRow},
	Executor, Pool, Postgres, Row,
};
use tracing::error;

use crate::{
	announcements::Announcement,
	attendance::AttendanceEntry,
	events::{Event, EventKind, EventUrgency, EventVisibility},
	member::{Member, MemberGroup, MemberKind, MemberMention},
	util::ToDropdown,
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

		let mut out = Self { pool };
		// Delete any empty members that may have been created
		if let Err(e) = out.delete_member("").await {
			error!("Failed to delete empty member: {e}");
		};

		Ok(out)
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

	async fn get_event(&self, event: &str) -> anyhow::Result<Option<Event>> {
		let mut result = sqlx::query("SELECT * FROM events WHERE Id = $1")
			.bind(event)
			.fetch(&self.pool);
		let row = result.try_next().await;
		match row {
			Ok(row) => {
				let Some(row) = row else {
					return Ok(None);
				};
				let event = read_event(event, row).context("Failed to read event")?;

				Ok(Some(event))
			}
			Err(e) => {
				error!("Failed to get event {event} from database: {e}");
				Err(anyhow!("Failed to get event from database"))
			}
		}
	}

	async fn create_event(&mut self, event: Event) -> anyhow::Result<()> {
		// Remove the existing event
		self.delete_event(&event.id)
			.await
			.context("Failed to delete existing event")?;
		sqlx::query("INSERT INTO events (Id, Name, Date, EndDate, Kind, Urgency, Visibility, Invites, RSVP) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)")
			.bind(event.id)
			.bind(event.name)
			.bind(event.date)
			.bind(event.end_date)
			.bind(event.kind.to_dropdown())
			.bind(event.urgency.to_dropdown())
			.bind(event.visibility.to_dropdown())
			.bind(
				event
					.invites
					.into_iter()
					.map(|x| x.to_db())
					.collect::<Vec<_>>(),
			)
			.bind(event.rsvp.into_iter().collect::<Vec<_>>())
			.execute(&self.pool)
			.await
			.context("Failed to create new event in database")?;

		Ok(())
	}

	async fn delete_event(&mut self, event: &str) -> anyhow::Result<()> {
		let query = sqlx::query("DELETE FROM events WHERE Id = $1").bind(event);

		query
			.execute(&self.pool)
			.await
			.context("Failed to remove event from database")?;

		Ok(())
	}

	async fn get_events(&self) -> anyhow::Result<impl Iterator<Item = Event>> {
		let result = sqlx::query("SELECT * FROM events")
			.fetch_all(&self.pool)
			.await;
		match result {
			Ok(rows) => {
				let mut out = Vec::with_capacity(rows.len());
				for row in rows {
					let id: String = row.try_get("id")?;
					let event = read_event(&id, row).context("Failed to read event")?;
					out.push(event);
				}

				Ok(out.into_iter())
			}
			Err(e) => {
				error!("Failed to get all events from database: {e}");
				Err(anyhow!("Failed to get events from database"))
			}
		}
	}

	async fn event_exists(&self, event: &str) -> anyhow::Result<bool> {
		let result = sqlx::query("SELECT 1 FROM events WHERE Id = $1")
			.bind(event)
			.execute(&self.pool)
			.await
			.context("Failed to query database for event")?;

		Ok(result.rows_affected() > 0)
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

	async fn get_attendance(&self, member: &str) -> anyhow::Result<Vec<AttendanceEntry>> {
		let result = sqlx::query("SELECT * FROM attendance WHERE Member = $1")
			.bind(member)
			.fetch_all(&self.pool)
			.await;
		match result {
			Ok(rows) => {
				let mut out = Vec::with_capacity(rows.len());
				for row in rows {
					let entry =
						read_attendance_entry(row).context("Failed to read attendance entry")?;
					out.push(entry);
				}

				Ok(out)
			}
			Err(e) => {
				error!("Failed to get all attendance from database: {e}");
				Err(anyhow!("Failed to get attendance from database"))
			}
		}
	}

	async fn get_current_attendance(
		&self,
		member: &str,
	) -> anyhow::Result<Option<AttendanceEntry>> {
		let result = sqlx::query("SELECT * FROM attendance WHERE Member = $1 AND EndDate IS NULL")
			.bind(member)
			.fetch_optional(&self.pool)
			.await
			.context("Failed to get current attendance from database")?;

		if let Some(row) = result {
			Ok(Some(
				read_attendance_entry(row).context("Failed to read entry")?,
			))
		} else {
			Ok(None)
		}
	}

	async fn record_attendance(&mut self, member: &str, event: &str) -> anyhow::Result<()> {
		// Make sure that any current attendance is finished
		self.finish_attendance(member)
			.await
			.context("Failed to finish existing attendance")?;
		if let Err(e) = sqlx::query(
			"INSERT INTO attendance (Member, StartDate, EndDate, Event) VALUES ($1, $2, $3, $4)",
		)
		.bind(member)
		.bind(Utc::now().to_rfc2822())
		.bind(None::<&str>)
		.bind(event)
		.execute(&self.pool)
		.await
		{
			error!("Failed to record attendance: {e}");
			return Err(anyhow!("Failed to record attendance in database"));
		}

		Ok(())
	}

	async fn finish_attendance(&mut self, member: &str) -> anyhow::Result<()> {
		sqlx::query("UPDATE attendance SET EndDate = $1 WHERE Member = $2 AND EndDate IS NULL")
			.bind(Utc::now().to_rfc2822())
			.bind(member)
			.execute(&self.pool)
			.await
			.context("Failed to finish attendance in database")?;

		Ok(())
	}
}

/// Setup the database
async fn setup_database(pool: &Pool<Postgres>) -> anyhow::Result<()> {
	pool.execute("CREATE TABLE IF NOT EXISTS members (Id text PRIMARY KEY, Name text, Kind text, Groups text[], Password text, PasswordSalt text, CreationDate text)")
		.await
		.context("Failed to set up members table")?;

	pool.execute("CREATE TABLE IF NOT EXISTS events (Id text PRIMARY KEY, Name text, Date text, EndDate text, Kind text, Urgency text, Visibility text, Invites text[], RSVP text[])")
		.await
		.context("Failed to set up events table")?;

	pool.execute("CREATE TABLE IF NOT EXISTS attendance (Id serial PRIMARY KEY, Member text, StartDate text, EndDate text, Event text)")
		.await
		.context("Failed to set up attendance table")?;

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

/// Read an event from the database
fn read_event(id: &str, row: PgRow) -> anyhow::Result<Event> {
	let name: &str = row.try_get("name")?;
	let date: &str = row.try_get("date")?;
	let end_date: Option<String> = row.try_get("enddate")?;
	let kind: &str = row.try_get("kind")?;
	let Ok(kind) = EventKind::from_str(kind) else {
		error!("Unknown event kind {kind}");
		return Err(anyhow!("Unknown event kind"));
	};
	let urgency: &str = row.try_get("urgency")?;
	let Ok(urgency) = EventUrgency::from_str(urgency) else {
		error!("Unknown event urgency {urgency}");
		return Err(anyhow!("Unknown event urgency"));
	};
	let visibility: &str = row.try_get("visibility")?;
	let Ok(visibility) = EventVisibility::from_str(visibility) else {
		error!("Unknown event urgency {visibility}");
		return Err(anyhow!("Unknown event visibility"));
	};
	let invites: Vec<String> = row.try_get("invites")?;
	let invites = invites
		.into_iter()
		.filter_map(|x| MemberMention::from_str(&x).ok());
	let rsvp: Vec<String> = row.try_get("rsvp")?;

	Ok(Event {
		id: id.to_string(),
		name: name.to_string(),
		date: date.to_string(),
		end_date,
		kind,
		urgency,
		visibility,
		invites: invites.collect(),
		rsvp: rsvp.into_iter().collect(),
	})
}

/// Read an attendance entry from the database
fn read_attendance_entry(row: PgRow) -> anyhow::Result<AttendanceEntry> {
	let start_date: String = row.try_get("startdate")?;
	let end_date: Option<String> = row.try_get("enddate")?;
	let event: String = row.try_get("event")?;

	Ok(AttendanceEntry {
		start_time: start_date,
		end_time: end_date,
		event,
	})
}
