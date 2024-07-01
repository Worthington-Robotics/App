#[rocket::get("/")]
pub fn index() -> String {
	"Hello from rocket!".into()
}
