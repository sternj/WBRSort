#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
use rocket::http::Status;
use rocket::response::status;
use rocket::response::NamedFile;
use rocket::State;
use rocket_contrib::templates::Template;
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

mod audio_excl;
use audio_excl::{init_map, FileLister};

struct AudioMX {
    mp: Mutex<HashMap<String, FileLister>>,
}

static GLOBAL_BASE: &str = "/home/sam/moods";
#[get("/<base_dir>")]
fn index(base_dir: String, audio_mx: State<AudioMX>) -> Result<Template, Status> {
    let mut context = HashMap::<String, String>::new();
    let mut map = audio_mx.mp.lock().expect("locking");
    let lister = match map.get_mut(&base_dir) {
        None => return Err(Status::new(404, "subdir not registered")),
        Some(x) => x,
    };
    let (filename, sec) = match lister.get_file() {
        Err(_) => return Err(Status::new(500, "error")),
        Ok(x) => x,
    };
    context.insert(String::from("base_dir"), String::from(base_dir));
    context.insert(String::from("audio_path"), String::from(filename));
    context.insert(String::from("audio_sec"), String::from(sec));
    Ok(Template::render("index", context))
}

// #[get("/random/<subdir>")]
// fn random(file_lister: State<Mutex<FileLister>>, subdir: String) -> String {}

#[get("/file/<subdir>/<name..>")]
fn get_file(subdir: String, name: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(GLOBAL_BASE).join(subdir).join(name)).ok()
}

fn setup(subdirs: Vec<String>, timeout_mins: u64) -> io::Result<rocket::Rocket> {
    let mp = init_map(
        PathBuf::from(GLOBAL_BASE),
        subdirs,
        Duration::from_secs(timeout_mins * 60),
    )?;
    return Ok(rocket::ignite()
        .mount("/", routes![index, get_file])
        .attach(Template::fairing())
        .manage(AudioMX { mp: Mutex::new(mp) }));
}
fn main() -> io::Result<()> {
    setup(vec![String::from("mouth")], 5)?.launch();
    Ok(())
}
