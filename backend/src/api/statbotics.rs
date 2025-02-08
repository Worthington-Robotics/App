use std::collections::HashMap;

use anyhow::Context;
use chrono::Utc;
use reqwest::Client;
use rocket::tokio::sync::RwLock;
use serde::{de::DeserializeOwned, Deserialize};

use crate::{events::get_season, scouting::TeamNumber};

/// Client used for Statbotics API requests
pub struct StatboticsClient {
	req_client: Client,
	epa_cache: RwLock<HashMap<TeamNumber, f32>>,
}

impl StatboticsClient {
	pub fn new(client: &Client) -> Self {
		Self {
			req_client: client.clone(),
			epa_cache: RwLock::new(HashMap::new()),
		}
	}

	/// Gets the EPA of a single team
	pub async fn get_epa(&self, team: TeamNumber) -> Option<f32> {
		self.epa_cache.read().await.get(&team).copied()
	}

	/// Update the EPA cache with values from the Statbotics server. If no season is provided, uses the default one.
	pub async fn get_stats(&self, season: Option<u32>) -> anyhow::Result<()> {
		let season = season.unwrap_or_else(|| get_season(&Utc::now()));
		let base_url = format!("https://api.statbotics.io/v3/team_years?year={}", season);

		let mut offset = 0;
		let mut teams = Vec::new();
		loop {
			// Make the first call to get the number of pages
			let response: Vec<StatboticsTeam> =
				self.call(&format!("{base_url}&offset={offset}")).await?;

			let len = response.len();
			teams.extend(response);

			// Since the API returns 1000 results at once, end once we get less than that
			if len < 1000 {
				break;
			}

			offset += 1000;
		}

		let mut lock = self.epa_cache.write().await;
		for team in teams {
			lock.insert(team.team, team.epa.total_points.mean.unwrap_or_default());
		}

		Ok(())
	}

	async fn call<D: DeserializeOwned>(&self, url: &str) -> anyhow::Result<D> {
		serde_json::from_slice(
			&self
				.req_client
				.get(url)
				.send()
				.await?
				.error_for_status()?
				.bytes()
				.await?,
		)
		.context("Failed to deserialize response")
	}
}

/// A team from the Statbotics API
#[derive(Deserialize)]
struct StatboticsTeam {
	team: TeamNumber,
	epa: EPA,
}

/// EPA stats from the API
#[derive(Deserialize)]
struct EPA {
	total_points: TotalPoints,
}

#[derive(Deserialize)]
struct TotalPoints {
	mean: Option<f32>,
}
