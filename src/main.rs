#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
use rocket::request::FlashMessage;
use rocket::request::Form;
use rocket::response::Flash;
use rocket::response::NamedFile;
use rocket::response::Redirect;
use rocket::State;
use rocket_contrib::templates::Template;
use serde::Serialize;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

mod audio_excl;
use audio_excl::{init_map, FileLister};

struct AudioMX {
    mp: Mutex<HashMap<String, FileLister>>,
    base_dir: String,
    dest_dir: String,
}

#[derive(Serialize)]
struct IndexCtx {
    dirs: Vec<String>,
    flash: Option<String>,
    flash_name: Option<String>,
}

#[derive(FromForm)]
struct AudioSubm {
    filename: String,
    sec: String,
}

// static GLOBAL_BASE: &str = "/home/sam/moods";
// static GLOBAL_DEST: &str = "/home/sam/moods-dest";

#[get("/")]
fn index(audio_mx: State<AudioMX>, flash: Option<FlashMessage>) -> Template {
    let mp = audio_mx.mp.lock().expect("locking");
    let dirs_vec = mp.keys().map(|x| x.to_string()).collect::<Vec<String>>();
    match flash {
        Some(m) => Template::render(
            "index",
            IndexCtx {
                dirs: dirs_vec,
                flash: Some(m.msg().to_string()),
                flash_name: Some(m.name().to_string()),
            },
        ),
        None => Template::render(
            "index",
            IndexCtx {
                dirs: dirs_vec,
                flash: None,
                flash_name: None,
            },
        ),
    }
}
#[get("/<subdir>")]
fn categorize_genre(
    subdir: String,
    audio_mx: State<AudioMX>,
    flash: Option<FlashMessage>,
) -> Result<Template, Flash<Redirect>> {
    let mut context = HashMap::<String, String>::new();
    let mut map = audio_mx.mp.lock().expect("locking");
    if let Some(msg) = flash {
        context.insert("flash_name".to_string(), msg.name().to_string());
        context.insert("flash".to_string(), msg.msg().to_string());
    }
    let lister = match map.get_mut(&subdir) {
        None => return Err(Flash::error(Redirect::to(uri![index]), "subdir not found")),
        Some(x) => x,
    };
    match lister.clean() {
        Err(e) => {
            println!("{}", e.to_string());
            return Err(Flash::error(Redirect::to(uri![index]), e.to_string()));
        }
        Ok(_) => (),
    };
    let (filename, sec) = match lister.get_file() {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            map.remove(&subdir);
            return Err(Flash::error(
                Redirect::to(uri![index]),
                "No more files in directory",
            ));
        }
        Err(e) => {
            println!("{}", e.to_string());
            return Err(Flash::error(Redirect::to(uri![index]), e.to_string()));
        }
        Ok(x) => x,
    };
    context.insert(String::from("subdir"), String::from(subdir));
    context.insert(String::from("audio_path"), String::from(filename));
    context.insert(String::from("audio_sec"), String::from(sec));
    Ok(Template::render("categorize_genre", context))
}

#[post("/submit/<subdir>?<accept>", data = "<form>")]
fn post_judgement(
    subdir: String,
    accept: bool,
    form: Form<AudioSubm>,
    audio_mx: State<AudioMX>,
) -> Result<Flash<Redirect>, Flash<Redirect>> {
    let mut map = audio_mx.mp.lock().expect("locking");
    let lister = match map.get_mut(&subdir) {
        None => {
            return Err(Flash::error(
                Redirect::to(uri![categorize_genre: subdir]),
                "Subdir not registered",
            ))
        }
        Some(x) => x,
    };
    let dest_dir = if accept { &subdir } else { "reject" };
    match lister.move_file_and_remove(
        &form.sec,
        PathBuf::from(&audio_mx.base_dir)
            .join(&subdir)
            .join(&form.filename),
        PathBuf::from(&audio_mx.dest_dir)
            .join(&dest_dir)
            .join(&form.filename),
    ) {
        Err(e) => {
            println!("{}", e.to_string());
            return Err(Flash::error(
                Redirect::to(uri![categorize_genre: subdir]),
                e.to_string(),
            ));
        }
        Ok(_) => (),
    }
    Ok(Flash::success(
        Redirect::to(uri![categorize_genre: subdir]),
        "success!",
    ))
}

#[get("/file/<subdir>/<name..>")]
fn get_file(subdir: String, name: PathBuf, audio_mx: State<AudioMX>) -> Option<NamedFile> {
    NamedFile::open(Path::new(&audio_mx.base_dir).join(subdir).join(name)).ok()
}

fn setup(
    subdirs: Vec<String>,
    base: &str,
    dest: &str,
    timeout_mins: u64,
) -> io::Result<rocket::Rocket> {
    let mp = init_map(
        PathBuf::from(base),
        subdirs,
        Duration::from_secs(timeout_mins * 60),
    )?;
    return Ok(rocket::ignite()
        .mount(
            "/",
            routes![categorize_genre, get_file, post_judgement, index],
        )
        .attach(Template::fairing())
        .manage(AudioMX {
            mp: Mutex::new(mp),
            base_dir: base.to_string(),
            dest_dir: dest.to_string(),
        }));
}

fn vec_of_subdirs(src_path: &PathBuf, dest_path: &PathBuf) -> io::Result<Vec<String>> {
    println!("reading folders from {:?}", src_path);
    let folder_names: Vec<String> = fs::read_dir(src_path)?
        .filter_map(io::Result::ok)
        .filter_map(|d| match d.file_type().ok() {
            None => None,
            Some(x) if x.is_dir() => Some(d),
            _ => None,
        })
        .filter(|x| match fs::read_dir(x.path()) {
            Ok(o) => o.count() > 0,
            Err(_) => false,
        })
        .filter_map(|x| x.file_name().into_string().ok())
        .collect();

    for name in &folder_names {
        println!("Trying to create {:?}", dest_path.join(&name));
        match fs::create_dir(dest_path.join(&name)) {
            Err(e) if e.kind() != io::ErrorKind::AlreadyExists => return Err(e),
            _ => (),
        }
        println!("{}", name);
    }
    println!("trying to create {:?}", dest_path.join("reject"));
    match fs::create_dir(dest_path.join("reject")) {
        Err(e) if e.kind() != io::ErrorKind::AlreadyExists => return Err(e),
        _ => (),
    }
    println!("created");
    Ok(folder_names)
}
fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let src = &args[1];
    let dest = &args[2];
    match vec_of_subdirs(&PathBuf::from(&src), &PathBuf::from(&dest)) {
        Ok(v) => setup(v, src, dest, 5)?.launch(),
        Err(e) => return Err(e),
    };
    // setup(vec![String::from("mouth")], 5)?.launch();
    Ok(())
}
