use std::fs;
use std::path::Path;
mod audio_excl;

fn main() {
    let p = Path::new(".");
    let dir = fs::read_dir(&p).expect("couldn't open");
    for f in dir {
        println!("{:?}", f)
    }
    println!("Hello, world!");
}
