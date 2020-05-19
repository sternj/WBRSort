#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
use rocket::http::Status;
use rocket::request::FlashMessage;
use rocket::request::Form;
use rocket::response::Flash;
use rocket::response::NamedFile;
use rocket::response::Redirect;
use rocket::State;
use rocket_contrib::templates::Template;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

mod audio_excl;
use audio_excl::{init_map, FileLister};

struct AudioMX {
    mp: Mutex<HashMap<String, FileLister>>,
}

#[derive(FromForm)]
struct AudioSubm {
    accept: bool,
    filename: String,
    sec: String,
}

static GLOBAL_BASE: &str = "/home/sam/moods";
static GLOBAL_DEST: &str = "/home/sam/moods-dest";

#[get("/<subdir>")]
fn index(
    subdir: String,
    audio_mx: State<AudioMX>,
    flash: Option<FlashMessage>,
) -> Result<Template, Status> {
    let mut context = HashMap::<String, String>::new();
    let mut map = audio_mx.mp.lock().expect("locking");
    if let Some(msg) = flash {
        context.insert("flash_name".to_string(), msg.name().to_string());
        context.insert("flash".to_string(), msg.msg().to_string());
    }
    let lister = match map.get_mut(&subdir) {
        None => return Err(Status::new(404, "subdir not registered")),
        Some(x) => x,
    };
    match lister.clean() {
        Err(e) => {
            println!("{}", e.to_string());
            return Err(Status::InternalServerError);
        }
        Ok(_) => (),
    };
    let (filename, sec) = match lister.get_file() {
        Err(e) => {
            println!("{}", e.to_string());
            return Err(Status::InternalServerError);
        }
        Ok(x) => x,
    };
    context.insert(String::from("subdir"), String::from(subdir));
    context.insert(String::from("audio_path"), String::from(filename));
    context.insert(String::from("audio_sec"), String::from(sec));
    Ok(Template::render("index", context))
}

#[post("/submit/<subdir>", data = "<form>")]
fn post_judgement(
    subdir: String,
    form: Form<AudioSubm>,
    audio_mx: State<AudioMX>,
) -> Result<Flash<Redirect>, Flash<Redirect>> {
    let mut map = audio_mx.mp.lock().expect("locking");
    let lister = match map.get_mut(&subdir) {
        None => {
            return Err(Flash::error(
                Redirect::to(uri![index: subdir]),
                "Subdir not registered",
            ))
        }
        Some(x) => x,
    };
    let dest_dir = if form.accept { &subdir } else { "reject" };
    match lister.move_file_and_remove(
        &form.sec,
        PathBuf::from(GLOBAL_BASE)
            .join(&subdir)
            .join(&form.filename),
        PathBuf::from(GLOBAL_DEST)
            .join(&dest_dir)
            .join(&form.filename),
    ) {
        Err(e) => {
            println!("{}", e.to_string());
            return Err(Flash::error(
                Redirect::to(uri![index: subdir]),
                e.to_string(),
            ));
        }
        Ok(_) => (),
    }
    Ok(Flash::success(
        Redirect::to(uri![index: subdir]),
        "success!",
    ))
}

#[get("/file/<subdir>/<name..>")]
fn get_file(subdir: String, name: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(&GLOBAL_BASE).join(subdir).join(name)).ok()
}

fn setup(subdirs: Vec<String>, timeout_mins: u64) -> io::Result<rocket::Rocket> {
    let mp = init_map(
        PathBuf::from(&GLOBAL_BASE),
        subdirs,
        Duration::from_secs(timeout_mins * 60),
    )?;
    return Ok(rocket::ignite()
        .mount("/", routes![index, get_file, post_judgement])
        .attach(Template::fairing())
        .manage(AudioMX { mp: Mutex::new(mp) }));
}

fn vec_of_subdirs(src_path: &PathBuf, dest_path: &PathBuf) -> io::Result<Vec<String>> {
    let folder_names: Vec<String> = fs::read_dir(src_path)?
        .filter_map(io::Result::ok)
        .filter_map(|d| match d.file_type().ok() {
            None => None,
            Some(x) if x.is_dir() => Some(d),
            _ => None,
        })
        .filter_map(|x| x.file_name().into_string().ok())
        .collect();

    for name in &folder_names {
        match fs::create_dir(dest_path.join(&name)) {
            Err(e) if e.kind() != io::ErrorKind::AlreadyExists => return Err(e),
            _ => (),
        }
        println!("{}", name);
    }
    match fs::create_dir(dest_path.join("reject")) {
        Err(e) if e.kind() != io::ErrorKind::AlreadyExists => return Err(e),
        _ => (),
    }
    Ok(folder_names)
}
fn main() -> io::Result<()> {
    setup(vec![String::from("mouth")], 5)?.launch();
    Ok(())
}
