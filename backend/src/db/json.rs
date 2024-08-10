#![allow(dead_code)]

use std::{
	collections::HashMap,
	fs::File,
	io::{BufReader, BufWriter},
	path::PathBuf,
};

use anyhow::Context;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::{
	announcements::Announcement, attendance::AttendanceEntry, events::Event, member::Member,
};

use super::Database;

pub struct JSONDatabase {
	contents: DatabaseContents,
	/// Whether or not to use an actual JSON file. Disabled for the caching database
	/// since everything just needs to be kept in memory
	write: bool,
}

impl JSONDatabase {
	pub fn new(write: bool) -> anyhow::Result<Self> {
		let path = Self::get_path();
		let contents = if write {
			if path.exists() {
				serde_json::from_reader(BufReader::new(
					File::open(path).context("Failed to open database file")?,
				))
				.context("Failed to deserialize contents")?
			} else {
				DatabaseContents::default()
			}
		} else {
			DatabaseContents::default()
		};

		Ok(Self { contents, write })
	}
}

impl Database for JSONDatabase {
	async fn open() -> anyhow::Result<Self> {
		Self::new(true)
	}

	async fn get_member(&self, id: &str) -> anyhow::Result<Option<Member>> {
		Ok(self.contents.members.get(id).cloned())
	}

	async fn create_member(&mut self, member: Member) -> anyhow::Result<()> {
		self.contents.members.insert(member.id.clone(), member);
		self.write()
	}

	async fn delete_member(&mut self, member: &str) -> anyhow::Result<()> {
		self.contents.members.remove(member);
		self.write()
	}

	async fn get_members(&self) -> anyhow::Result<impl Iterator<Item = Member>> {
		Ok(self.contents.members.values().cloned())
	}

	async fn member_exists(&self, member: &str) -> anyhow::Result<bool> {
		Ok(self.contents.members.contains_key(member))
	}

	async fn get_event(&self, id: &str) -> anyhow::Result<Option<Event>> {
		Ok(self.contents.events.get(id).cloned())
	}

	async fn create_event(&mut self, event: Event) -> anyhow::Result<()> {
		self.contents.events.insert(event.id.clone(), event);
		self.write()
	}

	async fn delete_event(&mut self, event: &str) -> anyhow::Result<()> {
		self.contents.events.remove(event);
		self.write()
	}

	async fn get_events(&self) -> anyhow::Result<impl Iterator<Item = Event>> {
		Ok(self.contents.events.values().cloned())
	}

	async fn event_exists(&self, event: &str) -> anyhow::Result<bool> {
		Ok(self.contents.events.contains_key(event))
	}

	async fn get_announcement(&self, announcement: &str) -> anyhow::Result<Option<Announcement>> {
		Ok(self.contents.announcements.get(announcement).cloned())
	}

	async fn create_announcement(&mut self, announcement: Announcement) -> anyhow::Result<()> {
		self.contents
			.announcements
			.insert(announcement.id.clone(), announcement);
		self.write()
	}

	async fn get_announcements(&self) -> anyhow::Result<impl Iterator<Item = Announcement>> {
		Ok(self.contents.announcements.values().cloned())
	}

	async fn read_announcement(&mut self, announcement: &str, member: &str) -> anyhow::Result<()> {
		if let Some(announcement) = self.contents.announcements.get_mut(announcement) {
			announcement.read.insert(member.to_string());
			self.write()
		} else {
			error!("Announcement does not exist");
			Ok(())
		}
	}

	async fn delete_announcement(&mut self, announcement: &str) -> anyhow::Result<()> {
		self.contents.announcements.remove(announcement);
		self.write()
	}

	async fn get_attendance(&self, member: &str) -> anyhow::Result<Vec<AttendanceEntry>> {
		Ok(self
			.contents
			.attendance
			.get(member)
			.cloned()
			.unwrap_or_default())
	}

	async fn get_current_attendance(
		&self,
		member: &str,
	) -> anyhow::Result<Option<AttendanceEntry>> {
		let Some(attendance) = self.contents.attendance.get(member) else {
			return Ok(None);
		};
		Ok(attendance.iter().find(|x| !x.is_complete()).cloned())
	}

	async fn record_attendance(&mut self, member: &str, event: &str) -> anyhow::Result<()> {
		self.contents
			.attendance
			.entry(member.to_string())
			.or_default()
			.push(AttendanceEntry {
				start_time: Utc::now().to_rfc2822(),
				end_time: None,
				event: event.to_string(),
			});
		self.write()
	}

	async fn finish_attendance(&mut self, member: &str) -> anyhow::Result<()> {
		let Some(entries) = self.contents.attendance.get_mut(member) else {
			return Ok(());
		};
		if let Some(entry) = entries.iter_mut().find(|x| !x.is_complete()) {
			entry.end_time = Some(Utc::now().to_rfc2822());
		}

		self.write()
	}
}

impl JSONDatabase {
	/// Debug the database by printing it out
	pub fn debug(&self) {
		dbg!(serde_json::to_string_pretty(&self.contents).unwrap());
	}

	fn get_path() -> PathBuf {
		PathBuf::from("./db.json")
	}

	fn write(&self) -> anyhow::Result<()> {
		if !self.write {
			return Ok(());
		}

		let path = Self::get_path();
		let mut file = BufWriter::new(File::create(path).context("Failed to open database file")?);
		serde_json::to_writer_pretty(&mut file, &self.contents)
			.context("Failed to write database contents")?;

		Ok(())
	}
}

#[derive(Serialize, Deserialize, Default)]
struct DatabaseContents {
	members: HashMap<String, Member>,
	events: HashMap<String, Event>,
	#[serde(default)]
	announcements: HashMap<String, Announcement>,
	#[serde(default)]
	attendance: HashMap<String, Vec<AttendanceEntry>>,
}
