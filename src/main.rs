#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
use rocket::response::NamedFile;
use rocket::State;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
mod audio_excl;
use audio_excl::{init_map, FileLister};

static GLOBAL_BASE: &str = "/test";
#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

#[get("/random/<subdir>")]
fn random(file_lister: State<FileLister>, subdir: String) -> String {}

// #[get("/file/<subdir>/<name..>")]
// fn get_file(subdir: String, name: PathBuf) -> Option<NamedFile> {}
fn setup(subdirs: Vec<String>, timeout_mins: i32) -> io::Result<rocket::Rocket> {}
fn main() {
    rocket::ignite().mount("/", routes![index]).launch();
}
