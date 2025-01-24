use std::{sync::Arc, time::Duration};

use anyhow::Context;
use rocket::{
	fairing::{Fairing, Info, Kind},
	tokio::{sync::RwLock, try_join},
	Orbit, Rocket,
};
use tracing::error;

use crate::{
	scouting::{matches::MatchNumber, TeamNumber},
	tasks::{Checklist, Task},
};

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
	for announcement in sql
		.get_announcements()
		.await
		.context("Failed to get announcements from database")?
	{
		cache.create_announcement(announcement).await?;
	}
	for checklist in sql
		.get_checklists()
		.await
		.context("Failed to get checklists from database")?
	{
		cache.create_checklist(checklist).await?;
	}
	for task in sql
		.get_tasks()
		.await
		.context("Failed to get tasks from database")?
	{
		cache.create_task(task).await?;
	}
	for team in sql
		.get_teams()
		.await
		.context("Failed to get teams from database")?
	{
		cache.create_team(team).await?;
	}
	for stats in sql
		.get_all_match_stats()
		.await
		.context("Failed to get match stats from database")?
	{
		cache.create_match_stats(stats).await?;
	}
	for status_update in sql
		.get_all_status()
		.await
		.context("Failed to get status updates from database")?
	{
		cache.update_team_status(status_update).await?;
	}
	for m in sql
		.get_matches()
		.await
		.context("Failed to get matches from database")?
	{
		cache.create_match(m).await?;
	}
	for assignment in sql
		.get_all_prescouting_assignments()
		.await
		.context("Failed to get prescouting assignments from database")?
	{
		cache.create_prescouting_assignment(assignment).await?;
	}
	for claims in sql
		.get_all_match_claims()
		.await
		.context("Failed to get match claims from database")?
	{
		cache.create_match_claims(claims).await?;
	}

	cache.set_global_data(sql.get_global_data().await?).await?;

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
		self.sql.delete_member(member).await?;
		self.cache.delete_member(member).await?;

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
		self.sql.delete_event(event).await?;
		self.cache.delete_event(event).await?;

		Ok(())
	}

	async fn get_events(&self) -> anyhow::Result<impl Iterator<Item = crate::events::Event>> {
		self.cache.get_events().await
	}

	async fn event_exists(&self, event: &str) -> anyhow::Result<bool> {
		self.cache.event_exists(event).await
	}

	async fn get_announcement(
		&self,
		announcement: &str,
	) -> anyhow::Result<Option<crate::announcements::Announcement>> {
		if let Some(announcement) = self.cache.get_announcement(announcement).await? {
			Ok(Some(announcement))
		} else {
			self.sql.get_announcement(announcement).await
		}
	}

	async fn create_announcement(
		&mut self,
		announcement: crate::announcements::Announcement,
	) -> anyhow::Result<()> {
		try_join!(
			self.sql.create_announcement(announcement.clone()),
			self.cache.create_announcement(announcement)
		)?;

		Ok(())
	}

	async fn get_announcements(
		&self,
	) -> anyhow::Result<impl Iterator<Item = crate::announcements::Announcement>> {
		self.cache.get_announcements().await
	}

	async fn read_announcement(&mut self, announcement: &str, member: &str) -> anyhow::Result<()> {
		try_join!(
			self.sql.read_announcement(announcement, member),
			self.cache.read_announcement(announcement, member)
		)?;

		Ok(())
	}

	async fn delete_announcement(&mut self, announcement: &str) -> anyhow::Result<()> {
		self.sql.delete_announcement(announcement).await?;
		self.cache.delete_announcement(announcement).await?;

		Ok(())
	}

	async fn get_attendance(
		&self,
		member: &str,
	) -> anyhow::Result<Vec<crate::attendance::AttendanceEntry>> {
		self.sql.get_attendance(member).await
	}

	async fn get_current_attendance(
		&self,
		member: &str,
	) -> anyhow::Result<impl Iterator<Item = crate::attendance::AttendanceEntry>> {
		self.sql.get_current_attendance(member).await
	}

	async fn record_attendance(&mut self, member: &str, event: &str) -> anyhow::Result<()> {
		self.sql.record_attendance(member, event).await
	}

	async fn finish_attendance(&mut self, member: &str, event: &str) -> anyhow::Result<()> {
		self.sql.finish_attendance(member, event).await
	}

	async fn get_checklist(&self, checklist: &str) -> anyhow::Result<Option<Checklist>> {
		self.cache.get_checklist(checklist).await
	}

	async fn create_checklist(&mut self, checklist: Checklist) -> anyhow::Result<()> {
		try_join!(
			self.sql.create_checklist(checklist.clone()),
			self.cache.create_checklist(checklist)
		)?;

		Ok(())
	}

	async fn delete_checklist(&mut self, checklist: &str) -> anyhow::Result<()> {
		self.sql.delete_checklist(checklist).await?;
		self.cache.delete_checklist(checklist).await?;

		Ok(())
	}

	async fn get_checklists(&self) -> anyhow::Result<impl Iterator<Item = Checklist>> {
		self.cache.get_checklists().await
	}

	async fn get_checklist_tasks(
		&self,
		checklist: &str,
	) -> anyhow::Result<impl Iterator<Item = Task>> {
		self.cache.get_checklist_tasks(checklist).await
	}

	async fn get_task(&self, task: &str) -> anyhow::Result<Option<Task>> {
		self.cache.get_task(task).await
	}

	async fn create_task(&mut self, task: Task) -> anyhow::Result<()> {
		try_join!(
			self.sql.create_task(task.clone()),
			self.cache.create_task(task)
		)?;

		Ok(())
	}

	async fn update_task(&mut self, task: &str) -> anyhow::Result<()> {
		try_join!(self.sql.update_task(task), self.cache.update_task(task))?;

		Ok(())
	}

	async fn delete_task(&mut self, task: &str) -> anyhow::Result<()> {
		self.sql.delete_task(task).await?;
		self.cache.delete_task(task).await?;

		Ok(())
	}

	async fn get_tasks(&self) -> anyhow::Result<impl Iterator<Item = Task>> {
		self.cache.get_tasks().await
	}

	async fn get_calendar(
		&self,
		calendar_id: &str,
	) -> anyhow::Result<Option<crate::member::Member>> {
		self.cache.get_calendar(calendar_id).await
	}

	async fn get_team(&self, id: TeamNumber) -> anyhow::Result<Option<crate::scouting::Team>> {
		if let Some(team) = self.cache.get_team(id).await? {
			Ok(Some(team))
		} else {
			self.sql.get_team(id).await
		}
	}

	async fn create_team(&mut self, team: crate::scouting::Team) -> anyhow::Result<()> {
		try_join!(
			self.sql.create_team(team.clone()),
			self.cache.create_team(team)
		)?;

		Ok(())
	}

	async fn delete_team(&mut self, team: TeamNumber) -> anyhow::Result<()> {
		self.sql.delete_team(team).await?;
		self.cache.delete_team(team).await?;

		Ok(())
	}

	async fn get_teams(&self) -> anyhow::Result<impl Iterator<Item = crate::scouting::Team>> {
		self.cache.get_teams().await
	}

	async fn create_match_stats(
		&mut self,
		stats: crate::scouting::matches::MatchStats,
	) -> anyhow::Result<()> {
		try_join!(
			self.sql.create_match_stats(stats.clone()),
			self.cache.create_match_stats(stats)
		)?;

		Ok(())
	}

	async fn get_all_match_stats(
		&self,
	) -> anyhow::Result<impl Iterator<Item = crate::scouting::matches::MatchStats>> {
		self.cache.get_all_match_stats().await
	}

	async fn get_team_info(
		&self,
		team: TeamNumber,
	) -> anyhow::Result<Option<crate::scouting::TeamInfo>> {
		if let Some(info) = self.cache.get_team_info(team).await? {
			Ok(Some(info))
		} else {
			self.sql.get_team_info(team).await
		}
	}

	async fn create_team_info(
		&mut self,
		team: TeamNumber,
		info: crate::scouting::TeamInfo,
	) -> anyhow::Result<()> {
		try_join!(
			self.sql.create_team_info(team, info.clone()),
			self.cache.create_team_info(team, info)
		)?;

		Ok(())
	}

	async fn get_all_team_info(
		&self,
	) -> anyhow::Result<impl Iterator<Item = crate::scouting::TeamInfo>> {
		self.cache.get_all_team_info().await
	}

	async fn get_auto(&self, id: &str) -> anyhow::Result<Option<crate::scouting::autos::Auto>> {
		if let Some(auto) = self.cache.get_auto(id).await? {
			Ok(Some(auto))
		} else {
			self.sql.get_auto(id).await
		}
	}

	async fn create_auto(&mut self, auto: crate::scouting::autos::Auto) -> anyhow::Result<()> {
		try_join!(
			self.sql.create_auto(auto.clone()),
			self.cache.create_auto(auto)
		)?;

		Ok(())
	}

	async fn delete_auto(&mut self, auto: &str) -> anyhow::Result<()> {
		self.sql.delete_auto(auto).await?;
		self.cache.delete_auto(auto).await?;

		Ok(())
	}

	async fn get_autos(
		&self,
		team: TeamNumber,
	) -> anyhow::Result<impl Iterator<Item = crate::scouting::autos::Auto>> {
		self.cache.get_autos(team).await
	}

	async fn get_team_status(
		&self,
		team: TeamNumber,
	) -> anyhow::Result<Vec<crate::scouting::status::StatusUpdate>> {
		self.cache.get_team_status(team).await
	}

	async fn update_team_status(
		&mut self,
		update: crate::scouting::status::StatusUpdate,
	) -> anyhow::Result<()> {
		try_join!(
			self.sql.update_team_status(update.clone()),
			self.cache.update_team_status(update)
		)?;

		Ok(())
	}

	async fn get_all_status(&self) -> anyhow::Result<Vec<crate::scouting::status::StatusUpdate>> {
		self.cache.get_all_status().await
	}

	async fn get_matches(
		&self,
	) -> anyhow::Result<impl Iterator<Item = crate::scouting::matches::Match>> {
		self.cache.get_matches().await
	}

	async fn create_match(&mut self, m: crate::scouting::matches::Match) -> anyhow::Result<()> {
		try_join!(self.sql.create_match(m.clone()), self.cache.create_match(m))?;

		Ok(())
	}

	async fn clear_matches(&mut self) -> anyhow::Result<()> {
		try_join!(self.sql.clear_matches(), self.cache.clear_matches())?;

		Ok(())
	}

	async fn get_prescouting_assignment(
		&self,
		assignment: &str,
	) -> anyhow::Result<Option<crate::scouting::assignment::ScoutingAssignment>> {
		self.cache.get_prescouting_assignment(assignment).await
	}

	async fn get_all_prescouting_assignments(
		&self,
	) -> anyhow::Result<impl Iterator<Item = crate::scouting::assignment::ScoutingAssignment>> {
		self.cache.get_all_prescouting_assignments().await
	}

	async fn create_prescouting_assignment(
		&mut self,
		assignment: crate::scouting::assignment::ScoutingAssignment,
	) -> anyhow::Result<()> {
		try_join!(
			self.sql.create_prescouting_assignment(assignment.clone()),
			self.cache.create_prescouting_assignment(assignment)
		)?;

		Ok(())
	}

	async fn get_match_claims(
		&self,
		m: &MatchNumber,
	) -> anyhow::Result<Option<crate::scouting::assignment::MatchClaims>> {
		self.cache.get_match_claims(m).await
	}

	async fn get_all_match_claims(
		&self,
	) -> anyhow::Result<impl Iterator<Item = crate::scouting::assignment::MatchClaims>> {
		self.cache.get_all_match_claims().await
	}

	async fn create_match_claims(
		&mut self,
		claims: crate::scouting::assignment::MatchClaims,
	) -> anyhow::Result<()> {
		try_join!(
			self.sql.create_match_claims(claims.clone()),
			self.cache.create_match_claims(claims)
		)?;

		Ok(())
	}

	async fn clear_match_claims(&mut self) -> anyhow::Result<()> {
		try_join!(
			self.sql.clear_match_claims(),
			self.cache.clear_match_claims()
		)?;

		Ok(())
	}

	async fn get_global_data(&self) -> anyhow::Result<super::GlobalData> {
		self.cache.get_global_data().await
	}

	async fn set_global_data(&mut self, data: super::GlobalData) -> anyhow::Result<()> {
		try_join!(
			self.sql.set_global_data(data.clone()),
			self.cache.set_global_data(data)
		)?;

		Ok(())
	}
}

/// Fairing for periodically syncing the cache
pub struct SyncCache {
	db: Arc<RwLock<CacheDatabase>>,
}

impl SyncCache {
	#[cfg(feature = "cachedb")]
	pub fn new(db: Arc<RwLock<CacheDatabase>>) -> Self {
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
				if let Err(e) = db.write().await.sync_cache().await {
					error!("Failed to sync cache: {e:#}");
				}
			}
		});
	}
}
