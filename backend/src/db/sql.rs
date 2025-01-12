#![allow(dead_code)]

use std::str::FromStr;

use anyhow::{anyhow, Context};
use chrono::Utc;
use rocket::{futures::TryStreamExt, tokio::try_join};
use sqlx::{
	postgres::{PgConnectOptions, PgPoolOptions, PgRow},
	Executor, Pool, Postgres, Row,
};
use tracing::error;

use crate::{
	announcements::Announcement,
	attendance::AttendanceEntry,
	events::{Event, EventKind, EventUrgency, EventVisibility},
	forms::Form,
	member::{Member, MemberGroup, MemberKind, MemberMention},
	scouting::{
		assignment::{MatchClaims, ScoutingAssignment},
		autos::Auto,
		matches::{Match, MatchNumber, MatchStats, MatchType},
		status::{RobotStatus, StatusUpdate},
		Competition, Division, Team, TeamInfo, TeamNumber,
	},
	tasks::{Checklist, Task},
	util::ToDropdown,
};

use super::{Database, GlobalData};

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
		sqlx::query("INSERT INTO members (Id, Name, Kind, Groups, Password, PasswordSalt, CreationDate, CalendarId, CompletedForms) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)")
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
			.bind(member.calendar_id)
			.bind(
				member
					.completed_forms
					.into_iter()
					.map(|x| x.to_db())
					.collect::<Vec<_>>(),
			)
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

	async fn get_announcement(&self, announcement: &str) -> anyhow::Result<Option<Announcement>> {
		let mut result = sqlx::query("SELECT * FROM announcements WHERE Id = $1")
			.bind(announcement)
			.fetch(&self.pool);
		let row = result.try_next().await;
		match row {
			Ok(row) => {
				let Some(row) = row else {
					return Ok(None);
				};
				let announcement =
					read_announcement(announcement, row).context("Failed to read announcement")?;

				Ok(Some(announcement))
			}
			Err(e) => {
				error!("Failed to get announcement {announcement} from database: {e}");
				Err(anyhow!("Failed to get announcement from database"))
			}
		}
	}

	async fn create_announcement(&mut self, announcement: Announcement) -> anyhow::Result<()> {
		sqlx::query("INSERT INTO announcements (Id, Title, Date, Body, Event, Mentioned, Read) VALUES ($1, $2, $3, $4, $5, $6, $7)")
			.bind(announcement.id)
			.bind(announcement.title)
			.bind(announcement.date)
			.bind(announcement.body)
			.bind(announcement.event)
			.bind(
				announcement
					.mentioned
					.into_iter()
					.map(|x| x.to_db())
					.collect::<Vec<_>>(),
			)
			.bind(
				announcement
					.read
					.into_iter()
					.collect::<Vec<_>>(),
			)
			.execute(&self.pool)
			.await
			.context("Failed to create new announcement in database")?;

		Ok(())
	}

	async fn get_announcements(&self) -> anyhow::Result<impl Iterator<Item = Announcement>> {
		let result = sqlx::query("SELECT * FROM announcements")
			.fetch_all(&self.pool)
			.await;
		match result {
			Ok(rows) => {
				let mut out = Vec::with_capacity(rows.len());
				for row in rows {
					let id: String = row.try_get("id")?;
					let announcement =
						read_announcement(&id, row).context("Failed to read announcement")?;
					out.push(announcement);
				}

				Ok(out.into_iter())
			}
			Err(e) => {
				error!("Failed to get all announcements from database: {e}");
				Err(anyhow!("Failed to get announcements from database"))
			}
		}
	}

	async fn read_announcement(&mut self, announcement: &str, member: &str) -> anyhow::Result<()> {
		let result = sqlx::query("UPDATE announcements SET Read = array_append(Read, $1) WHERE Id = $2 AND NOT $1 = ANY(Read)")
			.bind(member)
			.bind(announcement)
			.execute(&self.pool)
			.await;
		if let Err(e) = result {
			error!("Failed to read announcement in database: {e}");
			Err(anyhow!("Failed to read announcement in database"))
		} else {
			Ok(())
		}
	}

	async fn delete_announcement(&mut self, announcement: &str) -> anyhow::Result<()> {
		let result = sqlx::query("DELETE FROM announcements WHERE Id = $1")
			.bind(announcement)
			.execute(&self.pool)
			.await;
		if let Err(e) = result {
			error!("Failed to delete announcement from database: {e}");
			Err(anyhow!("Failed to delete announcement from database"))
		} else {
			Ok(())
		}
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

	async fn get_checklist(&self, checklist: &str) -> anyhow::Result<Option<Checklist>> {
		let mut result = sqlx::query("SELECT * FROM checklists WHERE Id = $1")
			.bind(checklist)
			.fetch(&self.pool);
		let row = result.try_next().await;
		match row {
			Ok(row) => {
				let Some(row) = row else {
					return Ok(None);
				};
				let checklist =
					read_checklist(checklist, row).context("Failed to read checklist")?;

				Ok(Some(checklist))
			}
			Err(e) => {
				error!("Failed to get checklist {checklist} from database: {e}");
				Err(anyhow!("Failed to get checklist from database"))
			}
		}
	}

	async fn create_checklist(&mut self, checklist: Checklist) -> anyhow::Result<()> {
		// Remove the existing checklist
		self.delete_checklist(&checklist.id)
			.await
			.context("Failed to delete existing checklist")?;
		sqlx::query("INSERT INTO checklists (Id, Name, Tasks) VALUES ($1, $2, $3)")
			.bind(checklist.id)
			.bind(checklist.name)
			.bind(checklist.tasks)
			.execute(&self.pool)
			.await
			.context("Failed to create new checklist in database")?;

		Ok(())
	}

	async fn delete_checklist(&mut self, checklist: &str) -> anyhow::Result<()> {
		let result = sqlx::query("DELETE FROM checklists WHERE Id = $1")
			.bind(checklist)
			.execute(&self.pool)
			.await;
		if let Err(e) = result {
			error!("Failed to delete checklist from database: {e}");
			Err(anyhow!("Failed to delete checklist from database"))
		} else {
			Ok(())
		}
	}

	async fn get_checklists(&self) -> anyhow::Result<impl Iterator<Item = Checklist>> {
		let result = sqlx::query("SELECT * FROM checklists")
			.fetch_all(&self.pool)
			.await;
		match result {
			Ok(rows) => {
				let mut out = Vec::with_capacity(rows.len());
				for row in rows {
					let id: String = row.try_get("id")?;
					let checklist = read_checklist(&id, row).context("Failed to read checklist")?;
					out.push(checklist);
				}

				Ok(out.into_iter())
			}
			Err(e) => {
				error!("Failed to get checklists from database: {e}");
				Err(anyhow!("Failed to get checklists from database"))
			}
		}
	}

	async fn get_checklist_tasks(
		&self,
		checklist: &str,
	) -> anyhow::Result<impl Iterator<Item = Task>> {
		let result = sqlx::query("SELECT * FROM tasks WHERE Checklist = $1")
			.bind(checklist)
			.fetch_all(&self.pool)
			.await;
		match result {
			Ok(rows) => {
				let mut out = Vec::with_capacity(rows.len());
				for row in rows {
					let id: String = row.try_get("id")?;
					let task = read_task(&id, row).context("Failed to read task")?;
					out.push(task);
				}

				Ok(out.into_iter())
			}
			Err(e) => {
				error!("Failed to get tasks from database: {e}");
				Err(anyhow!("Failed to get tasks from database"))
			}
		}
	}

	async fn get_task(&self, task: &str) -> anyhow::Result<Option<Task>> {
		let mut result = sqlx::query("SELECT * FROM tasks WHERE Id = $1")
			.bind(task)
			.fetch(&self.pool);
		let row = result.try_next().await;
		match row {
			Ok(row) => {
				let Some(row) = row else {
					return Ok(None);
				};
				let task = read_task(task, row).context("Failed to read task")?;

				Ok(Some(task))
			}
			Err(e) => {
				error!("Failed to get task {task} from database: {e}");
				Err(anyhow!("Failed to get task from database"))
			}
		}
	}

	async fn create_task(&mut self, task: Task) -> anyhow::Result<()> {
		// Remove the existing task
		self.delete_task(&task.id)
			.await
			.context("Failed to delete existing task")?;
		sqlx::query("INSERT INTO tasks (Id, Checklist, Text, Done) VALUES ($1, $2, $3, $4)")
			.bind(task.id)
			.bind(task.checklist)
			.bind(task.text)
			.bind(task.done)
			.execute(&self.pool)
			.await
			.context("Failed to create new task in database")?;

		Ok(())
	}

	async fn update_task(&mut self, task: &str) -> anyhow::Result<()> {
		let result = sqlx::query(
			"UPDATE tasks SET Done = (CASE WHEN Done = TRUE THEN FALSE ELSE TRUE END) WHERE Id = $1",
		)
		.bind(task)
		.execute(&self.pool)
		.await;
		if let Err(e) = result {
			error!("Failed to update task from database: {e}");
			Err(anyhow!("Failed to update task from database"))
		} else {
			Ok(())
		}
	}

	async fn delete_task(&mut self, task: &str) -> anyhow::Result<()> {
		let result = sqlx::query("DELETE FROM tasks WHERE Id = $1")
			.bind(task)
			.execute(&self.pool)
			.await;
		if let Err(e) = result {
			error!("Failed to delete task from database: {e}");
			Err(anyhow!("Failed to delete task from database"))
		} else {
			Ok(())
		}
	}

	async fn get_tasks(&self) -> anyhow::Result<impl Iterator<Item = Task>> {
		let result = sqlx::query("SELECT * FROM tasks")
			.fetch_all(&self.pool)
			.await;
		match result {
			Ok(rows) => {
				let mut out = Vec::with_capacity(rows.len());
				for row in rows {
					let id: String = row.try_get("id")?;
					let task = read_task(&id, row).context("Failed to read task")?;
					out.push(task);
				}

				Ok(out.into_iter())
			}
			Err(e) => {
				error!("Failed to get tasks from database: {e}");
				Err(anyhow!("Failed to get tasks from database"))
			}
		}
	}

	async fn get_calendar(&self, calendar_id: &str) -> anyhow::Result<Option<Member>> {
		let result = sqlx::query("SELECT * FROM members WHERE CalendarId = $1")
			.bind(calendar_id)
			.fetch_optional(&self.pool)
			.await
			.context("Failed to get current attendance from database")?;

		if let Some(row) = result {
			let id: String = row.try_get("id")?;
			Ok(Some(read_member(&id, row)?))
		} else {
			Ok(None)
		}
	}

	async fn get_team(&self, id: TeamNumber) -> anyhow::Result<Option<Team>> {
		let mut result = sqlx::query("SELECT * FROM teams WHERE Number = $1")
			.bind(id as i32)
			.fetch(&self.pool);
		let row = result.try_next().await;
		match row {
			Ok(row) => {
				let Some(row) = row else {
					return Ok(None);
				};
				let team = read_team(id, row).context("Failed to read team")?;

				Ok(Some(team))
			}
			Err(e) => {
				error!("Failed to get team {id} from database: {e}");
				Err(anyhow!("Failed to get team from database"))
			}
		}
	}

	async fn create_team(&mut self, team: Team) -> anyhow::Result<()> {
		// Remove the existing team
		self.delete_team(team.number)
			.await
			.context("Failed to delete existing team")?;
		sqlx::query(
			"INSERT INTO teams (Number, Name, RookieYear, Competitions, Followers) VALUES ($1, $2, $3, $4, $5)",
		)
		.bind(team.number as i32)
		.bind(team.name)
		.bind(team.rookie_year)
		.bind(
			team.competitions
				.into_iter()
				.map(|x| x.to_string())
				.collect::<Vec<_>>(),
		)
		.bind(team.followers.into_iter().collect::<Vec<_>>())
		.execute(&self.pool)
		.await
		.context("Failed to create new team in database")?;

		Ok(())
	}

	async fn delete_team(&mut self, team: TeamNumber) -> anyhow::Result<()> {
		let query = sqlx::query("DELETE FROM teams WHERE Number = $1").bind(team as i32);

		query
			.execute(&self.pool)
			.await
			.context("Failed to remove team from database")?;

		Ok(())
	}

	async fn get_teams(&self) -> anyhow::Result<impl Iterator<Item = Team>> {
		let result = sqlx::query("SELECT * FROM teams")
			.fetch_all(&self.pool)
			.await;
		match result {
			Ok(rows) => {
				let mut out = Vec::with_capacity(rows.len());
				for row in rows {
					let id = row.try_get::<i16, _>("number")? as TeamNumber;
					let team = read_team(id, row).context("Failed to read team")?;
					out.push(team);
				}

				Ok(out.into_iter())
			}
			Err(e) => {
				error!("Failed to get all teams from database: {e}");
				Err(anyhow!("Failed to get teams from database"))
			}
		}
	}

	async fn create_match_stats(&mut self, stats: MatchStats) -> anyhow::Result<()> {
		let serialized =
			serde_json::to_string(&stats).context("Failed to serialize match stats")?;
		sqlx::query("INSERT INTO match_stats (Team, Data) VALUES ($1, $2)")
			.bind(stats.team_number as i32)
			.bind(serialized)
			.execute(&self.pool)
			.await
			.context("Failed to create new match stats in database")?;

		Ok(())
	}

	async fn get_all_match_stats(&self) -> anyhow::Result<impl Iterator<Item = MatchStats>> {
		let result = sqlx::query("SELECT * FROM match_stats")
			.fetch_all(&self.pool)
			.await;
		match result {
			Ok(rows) => {
				let mut out = Vec::with_capacity(rows.len());
				for row in rows {
					let stats = read_match_stats(row).context("Failed to read match stats")?;
					out.push(stats);
				}

				Ok(out.into_iter())
			}
			Err(e) => {
				error!("Failed to get all match stats from database: {e}");
				Err(anyhow!("Failed to get match stats from database"))
			}
		}
	}

	async fn get_team_info(&self, team: TeamNumber) -> anyhow::Result<Option<TeamInfo>> {
		let mut result = sqlx::query("SELECT * FROM team_info WHERE Team = $1")
			.bind(team as i32)
			.fetch(&self.pool);
		let row = result.try_next().await;
		match row {
			Ok(row) => {
				let Some(row) = row else {
					return Ok(None);
				};
				let info = read_team_info(row).context("Failed to read team info")?;

				Ok(Some(info))
			}
			Err(e) => {
				error!("Failed to get team info for team {team} from database: {e}");
				Err(anyhow!("Failed to get team info from database"))
			}
		}
	}

	async fn create_team_info(&mut self, team: TeamNumber, info: TeamInfo) -> anyhow::Result<()> {
		sqlx::query("DELETE FROM team_info WHERE Team = $1")
			.bind(team as i32)
			.execute(&self.pool)
			.await
			.context("Failed to remove existing team info from database")?;

		let serialized = serde_json::to_string(&info).context("Failed to serialize team info")?;
		sqlx::query("INSERT INTO team_info (Team, Data) VALUES ($1, $2)")
			.bind(team as i32)
			.bind(serialized)
			.execute(&self.pool)
			.await
			.context("Failed to create new team info in database")?;

		Ok(())
	}

	async fn get_auto(&self, id: &str) -> anyhow::Result<Option<Auto>> {
		let mut result = sqlx::query("SELECT * FROM autos WHERE Id = $1")
			.bind(id)
			.fetch(&self.pool);
		let row = result.try_next().await;
		match row {
			Ok(row) => {
				let Some(row) = row else {
					return Ok(None);
				};
				let team = row.try_get::<i16, _>("team")? as TeamNumber;
				let auto = read_auto(id, team, row).context("Failed to read auto")?;

				Ok(Some(auto))
			}
			Err(e) => {
				error!("Failed to get auto {id} from database: {e}");
				Err(anyhow!("Failed to get auto from database"))
			}
		}
	}

	async fn create_auto(&mut self, auto: Auto) -> anyhow::Result<()> {
		// Remove the existing auto
		self.delete_auto(&auto.id)
			.await
			.context("Failed to delete existing auto")?;

		sqlx::query(
			"INSERT INTO autos (Id, Name, Team, Coral, Algae, Agitates, StartingPosition) VALUES ($1, $2, $3, $4, $5, $6, $7)",
		)
		.bind(auto.id)
		.bind(auto.name)
		.bind(auto.team as i32)
		.bind(auto.coral as i32)
		.bind(auto.algae as i32)
		.bind(auto.agitates)
		.bind(auto.starting_position)
		.execute(&self.pool)
		.await
		.context("Failed to create new auto in database")?;

		Ok(())
	}

	async fn delete_auto(&mut self, auto: &str) -> anyhow::Result<()> {
		let query = sqlx::query("DELETE FROM autos WHERE Id = $1").bind(auto);

		query
			.execute(&self.pool)
			.await
			.context("Failed to remove auto from database")?;

		Ok(())
	}

	async fn get_autos(&self, team: TeamNumber) -> anyhow::Result<impl Iterator<Item = Auto>> {
		let result = sqlx::query("SELECT * FROM autos WHERE Team = $1")
			.bind(team as i32)
			.fetch_all(&self.pool)
			.await;
		match result {
			Ok(rows) => {
				let mut out = Vec::with_capacity(rows.len());
				for row in rows {
					let id: String = row.try_get("id")?;
					let auto = read_auto(&id, team, row).context("Failed to read auto")?;
					out.push(auto);
				}

				Ok(out.into_iter())
			}
			Err(e) => {
				error!("Failed to get all autos from database: {e}");
				Err(anyhow!("Failed to get autos from database"))
			}
		}
	}

	async fn get_team_status(&self, team: TeamNumber) -> anyhow::Result<Vec<StatusUpdate>> {
		let result = sqlx::query("SELECT * FROM team_status WHERE Team = $1")
			.bind(team as i32)
			.fetch_all(&self.pool)
			.await;
		match result {
			Ok(rows) => {
				let mut out = Vec::with_capacity(rows.len());
				for row in rows {
					let status = read_status(team, row).context("Failed to read status update")?;
					out.push(status);
				}

				Ok(out)
			}
			Err(e) => {
				error!("Failed to get all status updates from database: {e}");
				Err(anyhow!("Failed to get status updates from database"))
			}
		}
	}

	async fn update_team_status(&mut self, update: StatusUpdate) -> anyhow::Result<()> {
		sqlx::query(
				"INSERT INTO team_status (Team, Date, Status, Details, Member) VALUES ($1, $2, $3, $4, $5)",
			)
			.bind(update.team as i32)
			.bind(update.date)
			.bind(update.status.to_db())
			.bind(update.details)
			.bind(update.member)
			.execute(&self.pool)
			.await
			.context("Failed to create new status update in database")?;

		Ok(())
	}

	async fn get_all_status(&self) -> anyhow::Result<Vec<StatusUpdate>> {
		let result = sqlx::query("SELECT * FROM team_status")
			.fetch_all(&self.pool)
			.await;
		match result {
			Ok(rows) => {
				let mut out = Vec::with_capacity(rows.len());
				for row in rows {
					let team = row.try_get::<i16, _>("team")? as TeamNumber;
					let status = read_status(team, row).context("Failed to read status update")?;
					out.push(status);
				}

				Ok(out)
			}
			Err(e) => {
				error!("Failed to get all status updates from database: {e}");
				Err(anyhow!("Failed to get status updates from database"))
			}
		}
	}

	async fn get_matches(&self) -> anyhow::Result<impl Iterator<Item = Match>> {
		let result = sqlx::query("SELECT * FROM matches")
			.fetch_all(&self.pool)
			.await;
		match result {
			Ok(rows) => {
				let mut out = Vec::with_capacity(rows.len());
				for row in rows {
					let m = read_match(row).context("Failed to read match")?;
					out.push(m);
				}

				Ok(out.into_iter())
			}
			Err(e) => {
				error!("Failed to get all matches from database: {e}");
				Err(anyhow!("Failed to get matches from database"))
			}
		}
	}

	async fn create_match(&mut self, m: Match) -> anyhow::Result<()> {
		sqlx::query(
				"INSERT INTO matches (Number, Type, Date, RedAlliance, BlueAlliance) VALUES ($1, $2, $3, $4, $5)",
			)
			.bind(m.num.num as i32)
			.bind(m.num.ty.to_string())
			.bind(m.date)
			.bind(m.red_alliance.into_iter().map(|x| x as i32).collect::<Vec<_>>())
			.bind(m.blue_alliance.into_iter().map(|x| x as i32).collect::<Vec<_>>())
			.execute(&self.pool)
			.await
			.context("Failed to create new match in database")?;

		Ok(())
	}

	async fn clear_matches(&mut self) -> anyhow::Result<()> {
		let query = sqlx::query("DELETE FROM matches");

		query
			.execute(&self.pool)
			.await
			.context("Failed to remove matches from database")?;

		Ok(())
	}

	async fn get_prescouting_assignment(
		&self,
		member: &str,
	) -> anyhow::Result<Option<ScoutingAssignment>> {
		let mut result = sqlx::query("SELECT * FROM prescouting_assignments WHERE Member = $1")
			.bind(member)
			.fetch(&self.pool);
		let row = result.try_next().await;
		match row {
			Ok(row) => {
				let Some(row) = row else {
					return Ok(None);
				};
				let assignment =
					read_assignment(member, row).context("Failed to read assignment")?;

				Ok(Some(assignment))
			}
			Err(e) => {
				error!("Failed to get assignment for member {member} from database: {e}");
				Err(anyhow!("Failed to get assignment from database"))
			}
		}
	}

	async fn get_all_prescouting_assignments(
		&self,
	) -> anyhow::Result<impl Iterator<Item = ScoutingAssignment>> {
		let result = sqlx::query("SELECT * FROM prescouting_assignments")
			.fetch_all(&self.pool)
			.await;
		match result {
			Ok(rows) => {
				let mut out = Vec::with_capacity(rows.len());
				for row in rows {
					let member: String = row.try_get("member")?;
					let assignment =
						read_assignment(&member, row).context("Failed to read assignment")?;
					out.push(assignment);
				}

				Ok(out.into_iter())
			}
			Err(e) => {
				error!("Failed to get prescouting assignments from database: {e}");
				Err(anyhow!(
					"Failed to get prescouting assignments from database"
				))
			}
		}
	}

	async fn create_prescouting_assignment(
		&mut self,
		assignment: ScoutingAssignment,
	) -> anyhow::Result<()> {
		let query = sqlx::query("DELETE FROM prescouting_assignments WHERE Member = $1")
			.bind(&assignment.member);

		query
			.execute(&self.pool)
			.await
			.context("Failed to remove existing assignment from database")?;

		sqlx::query("INSERT INTO prescouting_assignments (Member, Teams) VALUES ($1, $2)")
			.bind(assignment.member)
			.bind(
				assignment
					.teams
					.into_iter()
					.map(|x| x as i32)
					.collect::<Vec<_>>(),
			)
			.execute(&self.pool)
			.await
			.context("Failed to create new assignment in database")?;

		Ok(())
	}

	async fn get_match_claims(&self, m: &MatchNumber) -> anyhow::Result<Option<MatchClaims>> {
		let mut result = sqlx::query("SELECT * FROM match_claims WHERE Number = $1 AND Type = $2")
			.bind(m.num as i32)
			.bind(m.ty.to_string())
			.fetch(&self.pool);
		let row = result.try_next().await;
		match row {
			Ok(row) => {
				let Some(row) = row else {
					return Ok(None);
				};
				let claims = read_match_claims(row).context("Failed to read claims")?;

				Ok(Some(claims))
			}
			Err(e) => {
				error!("Failed to get claims for match {m} from database: {e}");
				Err(anyhow!("Failed to get claims from database"))
			}
		}
	}

	async fn get_all_match_claims(&self) -> anyhow::Result<impl Iterator<Item = MatchClaims>> {
		let result = sqlx::query("SELECT * FROM match_claims")
			.fetch_all(&self.pool)
			.await;
		match result {
			Ok(rows) => {
				let mut out = Vec::with_capacity(rows.len());
				for row in rows {
					let claims = read_match_claims(row).context("Failed to read claims")?;
					out.push(claims);
				}

				Ok(out.into_iter())
			}
			Err(e) => {
				error!("Failed to get match claims from database: {e}");
				Err(anyhow!("Failed to get match claims from database"))
			}
		}
	}

	async fn create_match_claims(&mut self, claims: MatchClaims) -> anyhow::Result<()> {
		let query = sqlx::query("DELETE FROM match_claims WHERE Number = $1 AND Type = $2")
			.bind(claims.m.num as i32)
			.bind(claims.m.ty.to_string());

		query
			.execute(&self.pool)
			.await
			.context("Failed to remove existing match claims from database")?;

		sqlx::query("INSERT INTO match_claims (Number, Type, Red1, Red2, Red3, Blue1, Blue2, Blue3) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)")
			.bind(claims.m.num as i32)
			.bind(claims.m.ty.to_string())
			.bind(claims.red_1)
			.bind(claims.red_2)
			.bind(claims.red_3)
			.bind(claims.blue_1)
			.bind(claims.blue_2)
			.bind(claims.blue_3)
			.execute(&self.pool)
			.await
			.context("Failed to create new match claims in database")?;

		Ok(())
	}

	async fn clear_match_claims(&mut self) -> anyhow::Result<()> {
		let query = sqlx::query("DELETE FROM match_claims");

		query
			.execute(&self.pool)
			.await
			.context("Failed to remove all match claims from database")?;

		Ok(())
	}

	async fn get_global_data(&self) -> anyhow::Result<GlobalData> {
		let mut result = sqlx::query("SELECT * FROM global_data").fetch(&self.pool);

		let row = result.try_next().await;
		match row {
			Ok(row) => {
				let Some(row) = row else {
					return Ok(GlobalData::default());
				};

				let data = read_global_data(row).context("Failed to read global data")?;

				Ok(data)
			}
			Err(e) => {
				error!("Failed to get global data from database: {e}");
				Err(anyhow!("Failed to get global data from database"))
			}
		}
	}

	async fn set_global_data(&mut self, data: GlobalData) -> anyhow::Result<()> {
		let query = sqlx::query("DELETE FROM global_data");

		query
			.execute(&self.pool)
			.await
			.context("Failed to remove existing global data from database")?;

		sqlx::query("INSERT INTO global_data (Competition, Division) VALUES ($1, $2)")
			.bind(data.current_competition.map(|x| x.to_string()))
			.bind(data.current_division.map(|x| x.to_string()))
			.execute(&self.pool)
			.await
			.context("Failed to create new global data in database")?;

		Ok(())
	}
}

/// Setup the database
async fn setup_database(pool: &Pool<Postgres>) -> anyhow::Result<()> {
	let members_task = pool.execute("CREATE TABLE IF NOT EXISTS members (Id text PRIMARY KEY, Name text, Kind text, Groups text[], Password text, PasswordSalt text, CreationDate text, CalendarId text, CompletedForms text[])");

	let events_task = pool.execute("CREATE TABLE IF NOT EXISTS events (Id text PRIMARY KEY, Name text, Date text, EndDate text, Kind text, Urgency text, Visibility text, Invites text[], RSVP text[])");

	let attendance_task = pool.execute("CREATE TABLE IF NOT EXISTS attendance (Id serial PRIMARY KEY, Member text, StartDate text, EndDate text, Event text)");

	let announcements_task = pool.execute("CREATE TABLE IF NOT EXISTS announcements (Id text PRIMARY KEY, Title text, Date text, Body text, Event text, Mentioned text[], Read text[])");

	let checklists_task = pool.execute(
		"CREATE TABLE IF NOT EXISTS checklists (Id text PRIMARY KEY, Name text, Tasks text[])",
	);

	let tasks_task = pool
		.execute("CREATE TABLE IF NOT EXISTS tasks (Id text PRIMARY KEY, Checklist text, Text text, Done bool)");

	let teams_task = pool.execute(
		"CREATE TABLE IF NOT EXISTS teams (Number int2 PRIMARY KEY, Name text, RookieYear int4, Competitions text[], Followers text[])",
	);

	let team_info_task =
		pool.execute("CREATE TABLE IF NOT EXISTS team_info (Team int2, Data text)");

	let match_stats_task =
		pool.execute("CREATE TABLE IF NOT EXISTS match_stats (Team int2, Data text)");

	let prescouting_assignments_task = pool.execute(
		"CREATE TABLE IF NOT EXISTS prescouting_assignments (Member text PRIMARY KEY, Teams int2[])",
	);

	let autos_task = pool.execute(
		"CREATE TABLE IF NOT EXISTS autos (Id text PRIMARY KEY, Name text, Team int2, Coral int2, Algae int2, Agitates bool, StartingPosition float4)",
	);

	let status_task = pool.execute(
		"CREATE TABLE IF NOT EXISTS team_status (Team int2, Date text, Status text, Details text, Member text)",
	);

	let matches_task = pool.execute(
		"CREATE TABLE IF NOT EXISTS matches (Number int4, Type text, Date text, RedAlliance int2[], BlueAlliance int2[])",
	);

	let match_claims_task = pool.execute(
		"CREATE TABLE IF NOT EXISTS match_claims (Number int4, Type text, Red1 text, Red2 text, Red3 text, Blue1 text, Blue2 text, Blue3 text)",
	);

	let global_data_task =
		pool.execute("CREATE TABLE IF NOT EXISTS global_data (Competition text, Division text)");

	try_join!(
		members_task,
		events_task,
		attendance_task,
		announcements_task,
		checklists_task,
		tasks_task,
		teams_task,
		team_info_task,
		match_stats_task,
		prescouting_assignments_task,
		autos_task,
		status_task,
		matches_task,
		match_claims_task,
		global_data_task,
	)
	.context("Failed to execute database setup tasks")?;

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
	let calendar_id: &str = row.try_get("calendarid")?;
	let forms: Option<Vec<String>> = row.try_get("completedforms")?;
	let forms = forms
		.unwrap_or_default()
		.into_iter()
		.filter_map(|x| Form::from_str(&x).ok());

	Ok(Member {
		id: id.to_string(),
		name: name.to_string(),
		kind,
		groups: groups.collect(),
		password: password.to_string(),
		password_salt,
		creation_date: creation_date.to_string(),
		calendar_id: calendar_id.to_string(),
		completed_forms: forms.collect(),
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

/// Read an announcement from the database
fn read_announcement(id: &str, row: PgRow) -> anyhow::Result<Announcement> {
	let title: String = row.try_get("title")?;
	let date: String = row.try_get("date")?;
	let body: Option<String> = row.try_get("body")?;
	let event: Option<String> = row.try_get("event")?;
	let mentioned: Vec<String> = row.try_get("mentioned")?;
	let mentioned = mentioned
		.into_iter()
		.filter_map(|x| MemberMention::from_str(&x).ok());
	let read: Vec<String> = row.try_get("read")?;

	Ok(Announcement {
		id: id.to_string(),
		title,
		date,
		body,
		event,
		mentioned: mentioned.collect(),
		read: read.into_iter().collect(),
	})
}

/// Read a checklist from the database
fn read_checklist(id: &str, row: PgRow) -> anyhow::Result<Checklist> {
	let name: String = row.try_get("name")?;
	let tasks: Vec<String> = row.try_get("tasks")?;

	Ok(Checklist {
		id: id.to_string(),
		name,
		tasks,
	})
}

/// Read a task from the database
fn read_task(id: &str, row: PgRow) -> anyhow::Result<Task> {
	let checklist: String = row.try_get("checklist")?;
	let text: String = row.try_get("text")?;
	let done: bool = row.try_get("done")?;

	Ok(Task {
		id: id.to_string(),
		checklist,
		text,
		done,
	})
}

/// Read a team from the database
fn read_team(id: TeamNumber, row: PgRow) -> anyhow::Result<Team> {
	let name: String = row.try_get("name")?;
	let rookie_year: i32 = row.try_get("rookieyear")?;
	let competitions: Vec<String> = row.try_get("competitions")?;
	let competitions = competitions
		.into_iter()
		.filter_map(|x| Competition::from_db(&x));

	let followers: Option<Vec<String>> = row.try_get("followers")?;
	let followers = followers.unwrap_or_default().into_iter();

	Ok(Team {
		number: id,
		name,
		rookie_year,
		competitions: competitions.collect(),
		followers: followers.collect(),
	})
}

/// Read match stats from the database
fn read_match_stats(row: PgRow) -> anyhow::Result<MatchStats> {
	let data: &str = row.try_get("data")?;
	let data = serde_json::from_str(data).context("Failed to deserialize data")?;

	Ok(data)
}

/// Read team info from the database
fn read_team_info(row: PgRow) -> anyhow::Result<TeamInfo> {
	let data: &str = row.try_get("data")?;
	let data = serde_json::from_str(data).context("Failed to deserialize data")?;

	Ok(data)
}

/// Read an auto from the database
fn read_auto(id: &str, team: TeamNumber, row: PgRow) -> anyhow::Result<Auto> {
	let name: String = row.try_get("name")?;
	let coral: i32 = row.try_get("coral")?;
	let algae: i32 = row.try_get("algae")?;
	let agitates: bool = row.try_get("agitates")?;
	let starting_position: f32 = row.try_get("startingposition")?;

	Ok(Auto {
		id: id.to_string(),
		name,
		team,
		coral: coral as u8,
		algae: algae as u8,
		agitates,
		starting_position,
	})
}

/// Read a status update from the database
fn read_status(id: TeamNumber, row: PgRow) -> anyhow::Result<StatusUpdate> {
	let date: String = row.try_get("date")?;
	let status: &str = row.try_get("status")?;
	let Ok(status) = RobotStatus::from_str(status) else {
		error!("Unknown robot status {status}");
		return Err(anyhow!("Unknown robot status"));
	};
	let details: String = row.try_get("details")?;
	let member: String = row.try_get("member")?;

	Ok(StatusUpdate {
		team: id,
		date,
		details,
		status,
		member,
	})
}

/// Read a match from the database
fn read_match(row: PgRow) -> anyhow::Result<Match> {
	let num: i32 = row.try_get("number")?;
	let ty: &str = row.try_get("type")?;
	let Ok(ty) = MatchType::from_str(ty) else {
		error!("Unknown match type {ty}");
		return Err(anyhow!("Unknown match type"));
	};
	let date: Option<String> = row.try_get("date")?;
	let red_alliance: Vec<i16> = row.try_get("redalliance")?;
	let red_alliance = red_alliance.into_iter().map(|x| x as TeamNumber).collect();
	let blue_alliance: Vec<i16> = row.try_get("bluealliance")?;
	let blue_alliance = blue_alliance.into_iter().map(|x| x as TeamNumber).collect();

	Ok(Match {
		num: MatchNumber {
			num: num as u16,
			ty,
		},
		date,
		red_alliance,
		blue_alliance,
	})
}

/// Read a scouting assignment from the database
fn read_assignment(member: &str, row: PgRow) -> anyhow::Result<ScoutingAssignment> {
	let teams: Vec<i16> = row.try_get("teams")?;
	let teams = teams.into_iter().map(|x| x as TeamNumber).collect();

	Ok(ScoutingAssignment {
		member: member.to_string(),
		teams,
	})
}

/// Read match claims from the database
fn read_match_claims(row: PgRow) -> anyhow::Result<MatchClaims> {
	let num: i32 = row.try_get("number")?;
	let ty: &str = row.try_get("type")?;
	let Ok(ty) = MatchType::from_str(ty) else {
		error!("Unknown match type {ty}");
		return Err(anyhow!("Unknown match type"));
	};
	let red_1: Option<String> = row.try_get("red1")?;
	let red_2: Option<String> = row.try_get("red2")?;
	let red_3: Option<String> = row.try_get("red3")?;
	let blue_1: Option<String> = row.try_get("blue1")?;
	let blue_2: Option<String> = row.try_get("blue2")?;
	let blue_3: Option<String> = row.try_get("blue3")?;

	Ok(MatchClaims {
		m: MatchNumber {
			num: num as u16,
			ty,
		},
		red_1,
		red_2,
		red_3,
		blue_1,
		blue_2,
		blue_3,
	})
}

/// Read global data from the database
fn read_global_data(row: PgRow) -> anyhow::Result<GlobalData> {
	let competition: Option<String> = row.try_get("competition")?;
	let competition = competition.and_then(|x| Competition::from_db(&x));
	let division: Option<String> = row.try_get("division")?;
	let division = division.and_then(|x| Division::from_db(&x));

	Ok(GlobalData {
		current_competition: competition,
		current_division: division,
	})
}
