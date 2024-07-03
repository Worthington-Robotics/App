use rocket::{response::content::RawCss, Responder};

#[rocket::get("/favicon.ico")]
pub fn favicon() -> Ico {
	Ico(include_bytes!("../assets/worbots-logo.ico"))
}

#[rocket::get("/assets/main.css")]
pub fn main_css() -> RawCss<&'static str> {
	RawCss(include_str!("../assets/main.css"))
}

#[rocket::get("/assets/rockwell_regular.otf")]
pub fn rockwell() -> &'static [u8] {
	include_bytes!("../assets/Rockwell Regular.otf")
}

#[rocket::get("/assets/icons/home.svg")]
pub fn icon_home() -> Svg {
	Svg(include_str!("../assets/icons/home.svg"))
}

#[derive(Responder)]
#[response(content_type = "image/x-icon")]
pub struct Ico(&'static [u8]);

#[derive(Responder)]
#[response(content_type = "image/svg+xml")]
pub struct Svg(&'static str);
