use std::{ffi::OsString, path::Path};

use minify_html::Cfg;

fn main() {
	println!("cargo::rerun-if-changed=src/assets");
	println!("cargo::rerun-if-changed=src/routes/components");
	println!("cargo::rerun-if-changed=src/routes/pages");
	println!("cargo::rerun-if-changed=build.rs");

	scan_dir("src/assets");
	scan_dir("src/routes/components");
	scan_dir("src/routes/pages");
}

fn scan_dir(dir: impl AsRef<Path>) {
	for asset in std::fs::read_dir(&dir).unwrap() {
		let asset = asset.unwrap();
		let file_type = asset.file_type().unwrap();
		let path = asset.path();
		if file_type.is_dir() {
			scan_dir(path);
		} else {
			if path.to_string_lossy().contains(".min.") {
				continue;
			}
			if let Some(extension) = path.extension() {
				let extension = extension.to_string_lossy();
				match extension.as_ref() {
					"html" | "css" => {
						let mut text = std::fs::read_to_string(&path).unwrap();
						// We have to wrap the css in style tags to trick the formatter into actually minifying it
						if extension.as_ref() == "css" {
							text = format!("<style>{text}</style>");
						}

						let mut cfg = Cfg::new();
						cfg.minify_css = true;
						cfg.minify_js = true;
						let mut out =
							String::from_utf8(minify_html::minify(text.as_bytes(), &cfg)).unwrap();

						if extension.as_ref() == "css" {
							out = out.replace("<style>", "");
							out = out.replace("</style>", "");
						}

						let new_file_name = format_min_file_name(&path);
						std::fs::write(dir.as_ref().join(new_file_name), out).unwrap();
					}
					_ => {}
				}
			}
		}
	}
}

fn format_min_file_name(path: &Path) -> OsString {
	let mut out = path.file_stem().unwrap().to_os_string();
	out.push(OsString::from(".min.").as_os_str());
	out.push(path.extension().unwrap());
	out
}
