use std::fmt::Display;

use chrono::NaiveDate;
use itertools::Itertools;
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
					"<option value=\"{}\" {selected_str}>{x}</option>",
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
