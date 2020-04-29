use rand;
use rand::seq::IteratorRandom;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::time::SystemTime;
use uuid::Uuid;

pub struct FileLister {
    files: Vec<PathBuf>,
    lock_acq: HashMap<PathBuf, SystemTime>,
    security: HashMap<PathBuf, String>,
}

impl FileLister {
    fn new(path: &Path) -> io::Result<FileLister> {
        let mut rng = rand::thread_rng();
        let num_items = fs::read_dir(&path)?.count();
        let items = fs::read_dir(&path)?
            .filter_map(io::Result::ok)
            .map(|s| s.path())
            .choose_multiple(&mut rng, num_items);

        return Ok(FileLister {
            files: items,
            lock_acq: HashMap::new(),
            security: HashMap::new(),
        });
    }

    fn clean(&mut self) -> io::Result<()> {
        let mut add_back: Vec<PathBuf> = vec![];
        self.lock_acq.retain(|path, modified| {
            let duration = match SystemTime::now().duration_since(*modified) {
                Err(_) => return false,
                Ok(t) => t,
            };
            if duration > Duration::from_secs(300) {
                add_back.push(path.to_path_buf());
                false
            } else {
                true
            }
        });
        self.files.append(&mut add_back);
        Ok(())
    }

    fn get_file(&mut self) -> io::Result<(File, String)> {
        // if opening item yields an error, just ditch it
        let filename = match self.files.pop() {
            Some(path) => path,
            None => return Err(io::Error::new(io::ErrorKind::NotFound, "No more files")),
        };
        let opened_file = File::open(&filename)?;
        self.lock_acq
            .insert(filename.to_path_buf(), SystemTime::now());
        let new_uuid = Uuid::new_v4();
        self.security
            .insert(filename.to_path_buf(), new_uuid.to_string());
        Ok((opened_file, new_uuid.to_string()))
    }

    fn move_file_and_remove(
        &mut self,
        sec_string: &str,
        file_to_move: PathBuf,
        new_dir: PathBuf,
    ) -> io::Result<()> {
        let expected_sec = match self.security.get(&file_to_move) {
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "No such file found",
                ))
            }
            Some(sec) => sec,
        };
        if expected_sec != sec_string {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "security string mismatch",
            ));
        }
        let new_path = new_dir.join(
            file_to_move
                .file_name()
                .expect("file name should not end with .."),
        );
        self.lock_acq.remove(&file_to_move);
        self.security.remove(&file_to_move);

        fs::rename(file_to_move, new_path)
    }
}

// impl AudioMutex {
//     fn get_random_file(&mut self, subdir: &str) -> Result<File, io::Error> {
//         let base_path = Path::new(&self.base_path).join(subdir);
//         let directory = fs::read_dir(&base_path)?;
//         let rnd_filename = match directory.choose(&mut self.rng).unwrap() {
//             Err(e) => return Err(e),
//             Ok(entry) => entry.path(),
//         };

//         let opened_file = File::open(base_path.join(rnd_filename))?;

//         return Ok(opened_file)
//     }
// }
