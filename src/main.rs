#![feature(proc_macro_hygiene, decl_macro)]

use std::collections::HashMap;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use std::{fs, io};

#[macro_use]
extern crate rocket;
use maplit::hashmap;
use rocket::request::{FlashMessage, Form};
use rocket::response::{Flash, NamedFile, Redirect};
use rocket::State;
use rocket_contrib::templates::Template;
use serde::Serialize;
use structopt::StructOpt;

mod audio_excl;
use audio_excl::{init_map, FileLister};

struct AudioMX(Mutex<HashMap<String, FileLister>>);

impl From<HashMap<String, FileLister>> for AudioMX {
    fn from(map: HashMap<String, FileLister>) -> Self {
        Self(Mutex::new(map))
    }
}

impl Deref for AudioMX {
    type Target = Mutex<HashMap<String, FileLister>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

trait DurationExt {
    fn from_mins(mins: u64) -> Self;
}

impl DurationExt for Duration {
    fn from_mins(mins: u64) -> Self {
        Self::from_secs(mins * 60)
    }
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

static GLOBAL_BASE: &str = "/home/sam/moods";
static GLOBAL_DEST: &str = "/home/sam/moods-dest";

#[get("/")]
fn index(audio_mx: State<AudioMX>, flash_message: Option<FlashMessage>) -> Template {
    let mp = audio_mx.lock().unwrap();
    let dirs = mp.keys().map(Clone::clone).collect();
    let flash = flash_message.as_ref().map(|m| m.msg().to_string());
    let flash_name = flash_message.as_ref().map(|m| m.name().to_string());
    Template::render(
        "index",
        IndexCtx {
            dirs,
            flash,
            flash_name,
        },
    )
}

#[get("/<subdir>")]
fn categorize_genre(
    subdir: String,
    audio_mx: State<AudioMX>,
    flash: Option<FlashMessage>,
) -> Result<Template, Flash<Redirect>> {
    let mut map = audio_mx.lock().unwrap();

    let lister = map
        .get_mut(&subdir)
        .ok_or_else(|| Flash::error(Redirect::to(uri![index]), "subdir not found"))?;

    lister.clean();

    let (filename, sec) = lister.get_file().map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            map.remove(&subdir);
            Flash::error(Redirect::to(uri![index]), "No more files in directory")
        } else {
            println!("{}", e.to_string());
            Flash::error(Redirect::to(uri![index]), e.to_string())
        }
    })?;

    let mut context = hashmap! {
        "subdir" => subdir,
        "audio_path" => filename,
        "audio_sec" => sec,
    };

    if let Some(msg) = flash {
        context.insert("flash_name", msg.name().to_string());
        context.insert("flash", msg.msg().to_string());
    }

    Ok(Template::render("categorize_genre", context))
}

#[post("/submit/<subdir>?<accept>", data = "<form>")]
fn post_judgement(
    subdir: String,
    accept: bool,
    form: Form<AudioSubm>,
    audio_mx: State<AudioMX>,
) -> Result<Flash<Redirect>, Flash<Redirect>> {
    let mut map = audio_mx.lock().unwrap();

    let lister = map.get_mut(&subdir).ok_or_else(|| {
        Flash::error(
            Redirect::to(uri![categorize_genre: &subdir]),
            "Subdir not registered",
        )
    })?;

    let dest_dir = if accept { &subdir } else { "reject" };

    lister
        .move_file_and_remove(
            &form.sec,
            PathBuf::from(GLOBAL_BASE)
                .join(&subdir)
                .join(&form.filename),
            PathBuf::from(GLOBAL_DEST)
                .join(&dest_dir)
                .join(&form.filename),
        )
        .map(|_| Flash::success(Redirect::to(uri![categorize_genre: &subdir]), "success!"))
        .map_err(|e| {
            println!("{}", e.to_string());
            Flash::error(Redirect::to(uri![categorize_genre: &subdir]), e.to_string())
        })
}

#[get("/file/<subdir>/<name..>")]
fn get_file(subdir: String, name: PathBuf) -> Option<NamedFile> {
    NamedFile::open(Path::new(&GLOBAL_BASE).join(subdir).join(name)).ok()
}

fn setup(subdirs: Vec<String>, timeout: Duration) -> io::Result<rocket::Rocket> {
    let mp = init_map(PathBuf::from(&GLOBAL_BASE), subdirs, timeout)?;
    return Ok(rocket::ignite()
        .mount(
            "/",
            routes![categorize_genre, get_file, post_judgement, index],
        )
        .attach(Template::fairing())
        .manage(AudioMX::from(mp)));
}

fn vec_of_subdirs(src_path: PathBuf, dest_path: PathBuf) -> io::Result<Vec<String>> {
    let folder_names: Vec<String> = fs::read_dir(src_path)?
        .filter_map(io::Result::ok)
        .filter_map(|subdir| {
            subdir
                .file_type()
                .ok()
                .filter(|ft| ft.is_dir())
                .map(|_| subdir)
        })
        .filter(|subdir| {
            fs::read_dir(subdir.path())
                .ok()
                .map_or(false, |dir| dir.count() > 0)
        })
        .filter_map(|subdir| subdir.file_name().into_string().ok())
        .collect();

    for name in &folder_names {
        fs::create_dir(dest_path.join(&name)).or_else(|e| {
            if e.kind() == io::ErrorKind::AlreadyExists {
                Ok(())
            } else {
                Err(e)
            }
        })?;
        println!("{}", name);
    }

    fs::create_dir(dest_path.join("reject")).or_else(|e| {
        if e.kind() == io::ErrorKind::AlreadyExists {
            Ok(())
        } else {
            Err(e)
        }
    })?;

    Ok(folder_names)
}

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(parse(from_os_str))]
    src: PathBuf,

    #[structopt(parse(from_os_str))]
    dest: PathBuf,
}

fn main() -> io::Result<()> {
    let opt = Opt::from_args();
    let subdirs = vec_of_subdirs(opt.src, opt.dest)?;
    setup(subdirs, Duration::from_mins(5))?.launch();
    Ok(())
}
