use crate::Result;
use crate::error::Error;
use std::fs;
use std::path::PathBuf;

pub struct MemoryStore {
    pub base_dir: PathBuf,
}

impl MemoryStore {
    pub fn new(base_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&base_dir).map_err(Error::Io)?;
        fs::create_dir_all(base_dir.join("episodes")).map_err(Error::Io)?;
        fs::create_dir_all(base_dir.join("knowledge")).map_err(Error::Io)?;
        fs::create_dir_all(base_dir.join("sessions")).map_err(Error::Io)?;
        Ok(MemoryStore { base_dir })
    }

    pub fn read(&self, rel_path: &str) -> Result<String> {
        fs::read_to_string(self.base_dir.join(rel_path)).map_err(Error::Io)
    }

    pub fn write(&self, rel_path: &str, content: &str) -> Result<()> {
        let full = self.base_dir.join(rel_path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).map_err(Error::Io)?;
        }
        fs::write(&full, content).map_err(Error::Io)
    }

    pub fn exists(&self, rel_path: &str) -> bool {
        self.base_dir.join(rel_path).exists()
    }

    pub fn list_dir(&self, rel_dir: &str) -> Result<Vec<String>> {
        let dir = self.base_dir.join(rel_dir);
        if !dir.exists() {
            return Ok(vec![]);
        }
        let mut names = vec![];
        for entry in fs::read_dir(&dir).map_err(Error::Io)? {
            let e = entry.map_err(Error::Io)?;
            names.push(e.file_name().to_string_lossy().into_owned());
        }
        Ok(names)
    }
}
