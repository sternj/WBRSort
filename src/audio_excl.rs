use rand;
use rand::seq::IteratorRandom;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;
use std::time::SystemTime;
use uuid::Uuid;

pub struct FileLister {
    files: Vec<PathBuf>,
    lock_acq: HashMap<PathBuf, SystemTime>,
    security: HashMap<PathBuf, String>,
    timeout: Duration,
}

impl FileLister {
    pub fn new<T: AsRef<Path>>(path: T, timeout: &Duration) -> io::Result<FileLister> {
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
            timeout: *timeout,
        });
    }

    pub fn clean(&mut self) -> io::Result<()> {
        let mut add_back: Vec<PathBuf> = vec![];
        let timeout = self.timeout.clone();
        self.lock_acq.retain(|path, modified| {
            let duration = match SystemTime::now().duration_since(*modified) {
                Err(_) => return false,
                Ok(t) => t,
            };
            if duration > timeout {
                add_back.push(path.to_path_buf());
                false
            } else {
                true
            }
        });
        self.files.append(&mut add_back);
        Ok(())
    }

    pub fn get_file(&mut self) -> io::Result<(String, String)> {
        // if opening item yields an error, just ditch it
        let filename = match self.files.pop() {
            Some(path) => path,
            None => return Err(io::Error::new(io::ErrorKind::NotFound, "No more files")),
        };
        self.lock_acq
            .insert(filename.to_path_buf(), SystemTime::now());
        let new_uuid = Uuid::new_v4();
        self.security
            .insert(filename.to_path_buf(), new_uuid.to_string());
        let name_w_extension = match filename.file_name() {
            None => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "found a filename ending with ..",
                ))
            }
            Some(p) => match p.to_str() {
                None => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Somehow this was invalid?",
                    ))
                }
                Some(q) => q,
            },
        };
        Ok((String::from(name_w_extension), new_uuid.to_string()))
    }

    pub fn move_file_and_remove(
        &mut self,
        sec_string: &str,
        file_to_move: PathBuf,
        new_dir: PathBuf,
    ) -> io::Result<()> {
        println!(
            "moving {} to {}",
            file_to_move.to_str().unwrap(),
            new_dir.to_str().unwrap()
        );
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

        self.lock_acq.remove(&file_to_move);
        self.security.remove(&file_to_move);

        fs::rename(file_to_move, new_dir)
    }
}

pub fn init_map(
    base_dir: PathBuf,
    subdirs: Vec<String>,
    timeout: Duration,
) -> io::Result<HashMap<String, FileLister>> {
    let mut ret: HashMap<String, FileLister> = HashMap::new();
    for subdir in subdirs {
        let lister = FileLister::new(base_dir.join(&subdir), &timeout)?;
        ret.insert(subdir, lister);
    }
    Ok(ret)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use tempdir::TempDir;
    #[test]
    fn test_lock_one() -> Result<(), io::Error> {
        let tmp_dir = TempDir::new("testing")?;
        let _file = File::create(tmp_dir.path().join("x"))?;
        let mut lister = FileLister::new(&tmp_dir, &Duration::from_secs(5))?;

        let (name, _): (String, String) = lister.get_file()?;
        assert_eq!(name, "x");
        assert!(lister.get_file().is_err(), "should not have any more files");
        Ok(())
    }

    #[test]
    fn test_cleanup() -> Result<(), io::Error> {
        let tmp_dir = TempDir::new("testing")?;
        let _file = File::create(tmp_dir.path().join("x"))?;
        let mut lister = FileLister::new(&tmp_dir, &Duration::from_secs(2))?;
        let (name, _): (String, String) = lister.get_file()?;
        assert_eq!(name, "x");
        std::thread::sleep(Duration::from_secs(3));
        lister.clean()?;
        let (name, _): (String, String) = lister.get_file()?;
        assert_eq!(name, "x");
        Ok(())
    }
}
