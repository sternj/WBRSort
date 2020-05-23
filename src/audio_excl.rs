use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use std::{fs, io};

use rand::{self, seq::SliceRandom};
use uuid::Uuid;

#[derive(Default)]
pub struct FileLister {
    files: Vec<PathBuf>,
    lock_acq: HashMap<PathBuf, SystemTime>,
    security: HashMap<PathBuf, String>,
    timeout: Duration,
}

impl FileLister {
    pub fn new<P: AsRef<Path>>(path: P, timeout: Duration) -> io::Result<FileLister> {
        let mut files = fs::read_dir(&path)?
            .filter_map(Result::ok)
            .map(|s| s.path())
            .collect::<Vec<_>>();
        files.shuffle(&mut rand::thread_rng());
        println!("Got items in order {:?}", files);
        Ok(FileLister {
            files,
            timeout,
            ..Default::default()
        })
    }

    pub fn clean(&mut self) {
        let mut add_back = Vec::new();
        let timeout = &self.timeout;
        self.lock_acq.retain(|path, modified| {
            SystemTime::now()
                .duration_since(*modified)
                .map(|duration| {
                    if duration > *timeout {
                        add_back.push(path.to_path_buf());
                        false
                    } else {
                        true
                    }
                })
                .unwrap_or(false)
        });
        self.files.append(&mut add_back);
    }

    pub fn get_file(&mut self) -> io::Result<(String, String)> {
        // if opening item yields an error, just ditch it
        let filename = self
            .files
            .pop()
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No more files"))?;
        self.lock_acq
            .insert(filename.to_path_buf(), SystemTime::now());
        let new_uuid = Uuid::new_v4();
        self.security
            .insert(filename.to_path_buf(), new_uuid.to_string());
        filename
            .file_name()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "found a filename ending with ..",
                )
            })?
            .to_str()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Filename {:?} isn't valid UTF-8.", &filename),
                )
            })
            .map(|name_w_extension| (name_w_extension.into(), new_uuid.to_string()))
    }

    pub fn move_file_and_remove(
        &mut self,
        sec_string: &str,
        file_to_move: PathBuf,
        new_dir: PathBuf,
    ) -> io::Result<()> {
        println!("moving {:?} to {:?}", file_to_move, new_dir);

        let expected_sec = self
            .security
            .get(&file_to_move)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No such file found"))?;

        if expected_sec != sec_string {
            Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "security string mismatch",
            ))
        } else {
            self.lock_acq.remove(&file_to_move);
            self.security.remove(&file_to_move);

            fs::rename(file_to_move, new_dir)
        }
    }
}

pub fn init_map(
    base_dir: PathBuf,
    subdirs: Vec<String>,
    timeout: Duration,
) -> io::Result<HashMap<String, FileLister>> {
    subdirs
        .into_iter()
        .map(|subdir| (base_dir.join(&subdir), subdir))
        .map(|(resolved_subdir, subdir)| Ok((subdir, FileLister::new(resolved_subdir, timeout)?)))
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs::File;
    use tempdir::TempDir;

    #[test]
    fn test_lock_one() -> Result<(), io::Error> {
        let tmp_dir = TempDir::new("testing")?;
        let _file = File::create(tmp_dir.path().join("x"))?;
        let mut lister = FileLister::new(&tmp_dir, Duration::from_secs(5))?;

        let (name, _): (String, String) = lister.get_file()?;
        assert_eq!(name, "x");
        assert!(lister.get_file().is_err(), "should not have any more files");
        Ok(())
    }

    #[test]
    fn test_cleanup() -> Result<(), io::Error> {
        let tmp_dir = TempDir::new("testing")?;
        let _file = File::create(tmp_dir.path().join("x"))?;
        let mut lister = FileLister::new(&tmp_dir, Duration::from_secs(2))?;
        let (name, _): (String, String) = lister.get_file()?;
        assert_eq!(name, "x");
        std::thread::sleep(Duration::from_secs(3));
        lister.clean();
        let (name, _): (String, String) = lister.get_file()?;
        assert_eq!(name, "x");
        Ok(())
    }
}
