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

use crate::{
	announcements::Announcement, attendance::AttendanceEntry, events::Event, member::Member,
};

use super::Database;

pub struct JSONDatabase {
	contents: DatabaseContents,
}

impl Database for JSONDatabase {
	async fn open() -> anyhow::Result<Self> {
		let path = Self::get_path();
		let contents = if path.exists() {
			serde_json::from_reader(BufReader::new(
				File::open(path).context("Failed to open database file")?,
			))
			.context("Failed to deserialize contents")?
		} else {
			DatabaseContents::default()
		};

		Ok(Self { contents })
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

	async fn get_members(&self) -> impl Iterator<Item = Member> {
		self.contents.members.values().cloned()
	}

	fn get_event(&self, id: &str) -> Option<Event> {
		self.contents.events.get(id).cloned()
	}

	fn create_event(&mut self, event: Event) -> anyhow::Result<()> {
		self.contents.events.insert(event.id.clone(), event);
		self.write()
	}

	fn get_events(&self) -> impl Iterator<Item = &Event> {
		self.contents.events.values()
	}

	fn get_announcement(&self, announcement: &str) -> Option<Announcement> {
		self.contents.announcements.get(announcement).cloned()
	}

	fn create_announcement(&mut self, announcement: Announcement) -> anyhow::Result<()> {
		self.contents
			.announcements
			.insert(announcement.id.clone(), announcement);
		self.write()
	}

	fn get_announcements(&self) -> impl Iterator<Item = &Announcement> {
		self.contents.announcements.values()
	}

	fn get_attendance(&self, member: &str) -> Vec<AttendanceEntry> {
		self.contents
			.attendance
			.get(member)
			.cloned()
			.unwrap_or_default()
	}

	fn get_current_attendance(&self, member: &str) -> Option<AttendanceEntry> {
		self.contents
			.attendance
			.get(member)?
			.iter()
			.find(|x| !x.is_complete())
			.cloned()
	}

	fn record_attendance(&mut self, member: &str, event: &str) -> anyhow::Result<()> {
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

	fn finish_attendance(&mut self, member: &str) -> anyhow::Result<()> {
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
