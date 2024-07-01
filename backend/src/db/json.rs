use std::{collections::HashMap, fs::File, io::BufReader, path::PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::member::Member;

use super::Database;

pub struct JSONDatabase {
	contents: DatabaseContents,
}

impl Database for JSONDatabase {
	fn open() -> anyhow::Result<Self> {
		let path = PathBuf::from("./db.json");
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

	fn get_member(&self, id: &str) -> Option<Member> {
		self.contents.members.get(id).cloned()
	}
}

impl JSONDatabase {
	/// Debug the database by printing it out
	pub fn debug(&self) {
		dbg!(serde_json::to_string_pretty(&self.contents).unwrap());
	}
}

#[derive(Serialize, Deserialize, Default)]
struct DatabaseContents {
	members: HashMap<String, Member>,
}
