use anyhow::Context;
use reqwest::Client;
use serde::{de::DeserializeOwned, Deserialize};

use crate::scouting::TeamNumber;

/// Client used for FIRST API requests
pub struct FirstClient {
	auth_header: String,
	req_client: Client,
}

impl FirstClient {
	pub fn new(client: &Client) -> Self {
		let api_token = std::env::var("FIRST_API_TOKEN")
			.expect("Failed to get FIRST API token from environment variable");
		let auth_header = format!("Basic {api_token}");
		Self {
			auth_header,
			req_client: client.clone(),
		}
	}

	/// Get a list of teams in the given season
	pub async fn get_teams(&self, season: i32) -> anyhow::Result<Vec<FirstTeam>> {
		// This is a paginated API so we have to go through quite a bit of work
		let base_url = format!("https://frc-api.firstinspires.org/v3.0/{season}/teams");
		// Make the first call to get the number of pages
		let response: TeamsAPIResponse = self.call(&base_url).await?;

		let mut teams = response.teams;
		teams.reserve_exact(response.team_count_total - teams.len());

		for i in 2..=response.page_total {
			let url = format!("{base_url}?page={i}");
			let response: TeamsAPIResponse = self.call(&url).await?;
			teams.extend(response.teams);
		}

		Ok(teams)
	}

	/// Get the qualification schedule for the given event
	pub async fn get_match_schedule(
		&self,
		season: i32,
		event: &str,
	) -> anyhow::Result<Vec<FirstMatch>> {
		let base_url = format!("https://frc-api.firstinspires.org/v3.0/{season}/schedule/{event}?tournamentLevel=Qualification");
		let response: MatchScheduleResponse = self.call(&base_url).await?;

		Ok(response.schedule)
	}

	async fn call<D: DeserializeOwned>(&self, url: &str) -> anyhow::Result<D> {
		serde_json::from_slice(
			&self
				.req_client
				.get(url)
				.header("Authorization", &self.auth_header)
				.send()
				.await?
				.error_for_status()?
				.bytes()
				.await?,
		)
		.context("Failed to deserialize response")
	}
}

/// A team from the FIRST API
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstTeam {
	pub team_number: TeamNumber,
	pub name_short: String,
	pub rookie_year: i32,
}

/// Response from the teams API
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeamsAPIResponse {
	teams: Vec<FirstTeam>,
	page_total: usize,
	team_count_total: usize,
}

/// Response from the match schedule API
#[derive(Deserialize)]
struct MatchScheduleResponse {
	#[serde(rename = "Schedule")]
	schedule: Vec<FirstMatch>,
}

/// A single scheduled match
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstMatch {
	pub match_number: u16,
	pub start_time: String,
	pub teams: Vec<FirstMatchTeam>,
}

/// A single team in a match
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FirstMatchTeam {
	pub team_number: TeamNumber,
}
