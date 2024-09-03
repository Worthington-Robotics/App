use std::fmt::Display;

use base64::{
	engine::{GeneralPurpose, GeneralPurposeConfig},
	Engine,
};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone};
use itertools::Itertools;
use rand::{rngs::StdRng, RngCore, SeedableRng};
use strum::IntoEnumIterator;

/// Trait for enums that can be converted into HTML select options
pub trait ToDropdown {
	fn to_dropdown(&self) -> &'static str;

	fn create_options(selected: Option<&Self>) -> String
	where
		Self: Display + IntoEnumIterator + PartialEq,
	{
		Self::iter()
			.map(|x| {
				let selected_str = if selected.is_some_and(|y| y == &x) {
					" selected"
				} else {
					""
				};
				format!(
					"<option value=\"{}\"{selected_str}>{x}</option>",
					x.to_dropdown()
				)
			})
			.join("")
	}
}

pub fn get_days_from_month(year: i32, month: u32) -> i64 {
	NaiveDate::from_ymd_opt(
		match month {
			12 => year + 1,
			_ => year,
		},
		match month {
			12 => 1,
			_ => month + 1,
		},
		1,
	)
	.unwrap()
	.signed_duration_since(NaiveDate::from_ymd_opt(year, month, 1).unwrap())
	.num_days()
}

/// Generate the ID for something like an event
pub fn generate_id() -> String {
	let mut rng = StdRng::from_entropy();
	let base64 = GeneralPurpose::new(&base64::alphabet::URL_SAFE, GeneralPurposeConfig::new());
	const LENGTH: usize = 32;
	let mut out = [0; LENGTH];
	for i in 0..LENGTH {
		out[i] = rng.next_u64() as u8;
	}

	base64.encode(out)
}

/// Render a nice date
pub fn render_date<T: TimeZone>(date: DateTime<T>) -> String
where
	T::Offset: Display,
{
	date.format("%a %B %d, %I:%M %p")
		.to_string()
		.replace(":00", "")
		.replace(" 0", " ")
}

/// Render a nice time
pub fn render_time<T: TimeZone>(date: DateTime<T>) -> String
where
	T::Offset: Display,
{
	date.format("%I:%M %p")
		.to_string()
		.replace(":00", "")
		.trim_start_matches("0")
		.to_owned()
}

/// Render a nice start and end date
pub fn render_date_range<T: TimeZone>(
	start_date: DateTime<T>,
	end_date: Option<DateTime<T>>,
) -> String
where
	T::Offset: Display,
{
	let within_same_day = end_date
		.as_ref()
		.is_some_and(|x| x.num_days_from_ce() == start_date.num_days_from_ce());
	let mut out = render_date(start_date);

	if let Some(end_date) = end_date {
		// Render the second date as just a time if it is within the same day
		let end_date = if within_same_day {
			render_time(end_date)
		} else {
			render_date(end_date)
		};
		out = format!("{out} - {end_date}");
	}

	out
}

/// Changes a number to one if it is zero
pub fn fix_zero(x: f32) -> f32 {
	if x == 0.0 {
		1.0
	} else {
		x
	}
}

/// Creates the attribute for a checkbox to say whether it is checked or not based on a boolean
pub fn checkbox_attr(val: bool) -> &'static str {
	if val {
		"checked"
	} else {
		""
	}
}

/// Creates the attribute for an HTML selection option to say whether it is selected or not based on a boolean
pub fn selected_attr(val: bool) -> &'static str {
	if val {
		"selected"
	} else {
		""
	}
}
