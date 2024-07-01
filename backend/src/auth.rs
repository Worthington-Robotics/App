use std::collections::HashMap;

use base64::{
	engine::{GeneralPurpose, GeneralPurposeConfig},
	Engine,
};
use rand::{rngs::StdRng, CryptoRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};

/// Manager for sessions
pub struct SessionManager {
	sessions: HashMap<String, Session>,
	rng: StdRng,
	base64: GeneralPurpose,
}

impl SessionManager {
	/// Create a new SessionManager
	pub fn new() -> Self {
		Self {
			sessions: HashMap::new(),
			rng: StdRng::from_entropy(),
			base64: GeneralPurpose::new(&base64::alphabet::STANDARD, GeneralPurposeConfig::new()),
		}
	}

	/// Create a new session, returning it's ID
	pub fn new_session(&mut self, member_id: &str) -> String {
		let session_id = generate_session_id(&mut self.rng, &mut self.base64);

		let session = Session {
			member: member_id.to_string(),
		};

		self.sessions.insert(session_id.clone(), session);

		session_id
	}

	/// Get a session
	pub fn get_session(&self, session_id: &str) -> Option<&Session> {
		self.sessions.get(session_id)
	}
}

/// Generate the ID for a new session
fn generate_session_id<R>(rng: &mut R, base64: &mut GeneralPurpose) -> String
where
	R: Rng + CryptoRng,
{
	const LENGTH: usize = 128;
	let mut out = [0; LENGTH];
	for i in 0..LENGTH {
		out[i] = rng.next_u64() as u8;
	}

	base64.encode(out)
}

/// A single session
pub struct Session {
	/// The member ID associated with this session
	pub member: String,
}

/// Privilege level of a user or session
#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum Privilege {
	Standard,
	Elevated,
}
