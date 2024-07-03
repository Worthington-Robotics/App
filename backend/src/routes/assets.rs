use rocket::response::content::RawCss;

#[rocket::get("/favicon.ico")]
pub fn favicon() -> &'static [u8] {
	include_bytes!("../assets/worbots-logo.ico")
}

#[rocket::get("/assets/main.css")]
pub fn main_css() -> RawCss<&'static str> {
	RawCss(include_str!("../assets/main.css"))
}

#[rocket::get("/assets/rockwell_regular.otf")]
pub fn rockwell() -> &'static [u8] {
	include_bytes!("../assets/Rockwell Regular.otf")
}
