use rocket::{
	http::Header,
	response::{
		content::{RawCss, RawJavaScript},
		Responder,
	},
};

// We don't have to cache this as most browsers do anyway
#[rocket::get("/favicon.ico")]
pub fn favicon() -> Ico {
	Ico(include_bytes!("../assets/worbots-logo.ico"))
}

#[rocket::get("/assets/main.css")]
pub fn main_css() -> RawCss<&'static str> {
	RawCss(include_str!("../assets/main.min.css"))
}

#[rocket::get("/assets/static16.css")]
pub fn static_css() -> CacheFor<RawCss<&'static str>> {
	CacheFor(RawCss(include_str!("../assets/static.min.css")), ONE_WEEK)
}

#[rocket::get("/assets/sortable.min.js")]
pub fn sortable_js() -> CacheFor<RawJavaScript<&'static str>> {
	CacheFor(
		RawJavaScript(include_str!("../assets/sortable.min.js")),
		ONE_YEAR,
	)
}

#[rocket::get("/assets/error2.js")]
pub fn error_js() -> CacheFor<RawJavaScript<&'static str>> {
	CacheFor(RawJavaScript(include_str!("../assets/error.js")), ONE_YEAR)
}

#[rocket::get("/assets/prompt.js")]
pub fn prompt_js() -> CacheFor<RawJavaScript<&'static str>> {
	CacheFor(
		RawJavaScript(include_str!("../assets/scripts/prompt.js")),
		ONE_WEEK,
	)
}

#[rocket::get("/assets/logo-gears.svg")]
pub fn logo() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/logo-gears.svg")), ONE_YEAR)
}

#[rocket::get("/assets/rockwell_regular.otf")]
pub fn rockwell() -> CacheFor<&'static [u8]> {
	CacheFor(include_bytes!("../assets/Rockwell Regular.otf"), ONE_YEAR)
}

#[rocket::get("/assets/icons/home.svg")]
pub fn icon_home() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/home.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/clock.svg")]
pub fn icon_clock() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/clock.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/plus.svg")]
pub fn icon_plus() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/plus.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/mail.svg")]
pub fn icon_mail() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/mail.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/edit.svg")]
pub fn icon_edit() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/edit.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/delete2.svg")]
pub fn icon_delete() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/delete.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/check.svg")]
pub fn icon_check() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/check.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/box.svg")]
pub fn icon_box() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/box.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/eye.svg")]
pub fn icon_eye() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/eye.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/star.svg")]
pub fn icon_star() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/star.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/star_outline.svg")]
pub fn icon_star_outline() -> CacheFor<Svg> {
	CacheFor(
		Svg(include_bytes!("../assets/icons/star_outline.svg")),
		ONE_WEEK,
	)
}

#[rocket::get("/assets/icons/user.svg")]
pub fn icon_user() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/user.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/calendar.svg")]
pub fn icon_calendar() -> CacheFor<Svg> {
	CacheFor(
		Svg(include_bytes!("../assets/icons/calendar.svg")),
		ONE_WEEK,
	)
}

#[rocket::get("/assets/icons/location.svg")]
pub fn icon_location() -> CacheFor<Svg> {
	CacheFor(
		Svg(include_bytes!("../assets/icons/location.svg")),
		ONE_WEEK,
	)
}

#[rocket::get("/assets/icons/coral.svg")]
pub fn icon_coral() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/coral.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/algae.svg")]
pub fn icon_algae() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/algae.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/error.svg")]
pub fn icon_error() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/error.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/window.svg")]
pub fn icon_window() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/window.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/hashtag.svg")]
pub fn icon_hashtag() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/hashtag.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/download.svg")]
pub fn icon_download() -> CacheFor<Svg> {
	CacheFor(
		Svg(include_bytes!("../assets/icons/download.svg")),
		ONE_WEEK,
	)
}

#[rocket::get("/assets/icons/shield.svg")]
pub fn icon_shield() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/shield.svg")), ONE_WEEK)
}

#[rocket::get("/assets/icons/upload.svg")]
pub fn icon_upload() -> CacheFor<Svg> {
	CacheFor(Svg(include_bytes!("../assets/icons/upload.svg")), ONE_WEEK)
}

#[derive(Responder)]
#[response(content_type = "image/x-icon")]
pub struct Ico(pub &'static [u8]);

#[derive(Responder)]
#[response(content_type = "image/svg+xml")]
pub struct Svg(pub &'static [u8]);

#[derive(Responder)]
#[response(content_type = "image/svg+xml")]
pub struct SvgDynamic(pub Vec<u8>);

#[derive(Responder)]
#[response(content_type = "image/png")]
pub struct Png(pub &'static [u8]);

/// Simple responder to set cache headers for asset responses
pub struct CacheFor<R>(pub R, pub usize);

impl<'r, 'o: 'r, R: Responder<'r, 'o>> Responder<'r, 'o> for CacheFor<R> {
	fn respond_to(self, request: &'r rocket::Request<'_>) -> rocket::response::Result<'o> {
		let mut out = self.0.respond_to(request);
		if let Ok(out) = &mut out {
			let control = format!("max-age={}, public", self.1);
			out.set_header(Header::new("Cache-Control", control));
		}

		out
	}
}

pub const ONE_YEAR: usize = 31536000;
pub const ONE_WEEK: usize = 604800;
