use std::{f32::consts::PI, fmt::Display};

use anyhow::Context;
use base64::{
	engine::{GeneralPurpose, GeneralPurposeConfig},
	Engine,
};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use chrono_tz::{Tz, US::Eastern};
use itertools::Itertools;
use rand::{rngs::StdRng, RngCore, SeedableRng};
use strum::IntoEnumIterator;

/// Global timezone
pub static TIMEZONE: &Tz = &Eastern;

/// Trait for enums that can be converted into HTML select options
pub trait ToDropdown {
	fn to_dropdown(&self) -> &'static str;

	/// Create dropdown options, along with an optional none option
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
	let month = get_short_month(date.month());
	date.format("%a MONTH %d, %I:%M %p")
		.to_string()
		.replace("MONTH", month)
		.replace(":00", "")
		.replace(" 0", " ")
}

/// Gets the abbreviated month from a month number
pub fn get_short_month(month: u32) -> &'static str {
	match month {
		1 => "Jan",
		2 => "Feb",
		3 => "Mar",
		4 => "Apr",
		5 => "May",
		6 => "Jun",
		7 => "Jul",
		8 => "Aug",
		9 => "Sep",
		10 => "Oct",
		11 => "Nov",
		12 => "Dec",
		_ => "???",
	}
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

/// Parses a date from JS/HTML's version
pub fn date_from_js(date: String, is_utc: bool) -> anyhow::Result<DateTime<Utc>> {
	let year = date[0..4].parse().context("Failed to parse year")?;
	let month = date[5..7].parse().context("Failed to parse month")?;
	let day = date[8..10].parse().context("Failed to parse day")?;
	let hour = date[11..13]
		.parse::<u32>()
		.context("Failed to parse hour")?;

	let min = date[14..16].parse().context("Failed to parse minute")?;

	// If the date fails, then we know that we overflowed the day from wrapping the hour. Try wrapping the day now.
	let naive_date = NaiveDate::from_ymd_opt(year, month, day).context("Failed to create date")?;

	let naive_dt = naive_date
		.and_hms_opt(hour, min, 0)
		.context("Failed to add time to date")?;

	// Parse from the specified timezone
	let out = if is_utc {
		naive_dt.and_utc()
	} else {
		TIMEZONE
			.from_local_datetime(&naive_dt)
			.earliest()
			.context("Failed to solve to a single Datetime")?
			.to_utc()
	};

	Ok(out)
}

/// Changes a number to one if it is zero
pub fn fix_zero(x: f32) -> f32 {
	if x == 0.0 {
		1.0
	} else {
		x
	}
}

/// Changes an empty string to a quoted one
pub fn fix_empty_string(string: &str) -> &str {
	if string.is_empty() {
		"\"\""
	} else {
		string
	}
}

/// Escapes quotes in a string with backslashes
pub fn escape_quotes(string: &str) -> String {
	string.replace("\"", "\\\"")
}

/// Escapes quotes and <> signs in a string with backslashes
pub fn escape_html(string: &str) -> String {
	escape_quotes(&string.replace("<", "\\<").replace(">", "\\>"))
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

/// Renders a progress ring
pub fn render_progress_ring(size: f32, progress: f32) -> String {
	let out = include_str!("routes/components/ui/ring.min.html");
	let radius = size * 0.25;
	let out = out.replace("{{size}}", &size.to_string());
	let out = out.replace("{{radius}}", &radius.to_string());
	let out = out.replace("{{midpoint}}", &(size / 2.0).to_string());
	let circumference = radius * 2.0 * PI;
	let out = out.replace("__circumference__", &circumference.to_string());
	let dash_offset = circumference - progress * circumference;
	let out = out.replace("__dash-offset__", &dash_offset.to_string());

	out
}

/// Calculates standard deviation
pub fn standard_deviation(values: &[f32], mean: f32) -> f32 {
	if values.is_empty() {
		return 0.0;
	}

	let mut sum = 0.0;
	for value in values {
		sum += (mean - value).powi(2);
	}

	(sum / values.len() as f32).sqrt()
}

/// Why can't floats just be ord
pub fn float_max(values: impl Iterator<Item = f32>) -> Option<f32> {
	values.max_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal))
}
