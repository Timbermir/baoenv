use std::{
    env,
    fs,
    io::{self, Write},
    path::{Path, PathBuf}
};

use serde::{Deserialize, Serialize};

use crate::error::{BaoError, BaoResult};

#[derive(Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub addr: Option<String>,
    pub access: Option<bool>,
    pub mount: Option<String>,
    pub role: Option<String>,
    pub credentials: Option<PathBuf>
}

impl Config {
    pub fn directory() -> BaoResult<PathBuf> {
        Ok(dirs::config_dir()
            .ok_or(BaoError::NoConfigDirectory)?
            .join("baoenv"))
    }

    pub fn load() -> BaoResult<Self> {
        match fs::read_to_string(Self::directory()?.join("config.toml")) {
            Ok(text) => Ok(toml::from_str(&text)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error.into())
        }
    }

    pub fn save(&self) -> BaoResult<()> {
        let directory = Self::directory()?;
        fs::create_dir_all(&directory)?;
        Ok(fs::write(
            directory.join("config.toml"),
            toml::to_string(self)?
        )?)
    }

    pub fn credentials(&self) -> BaoResult<PathBuf> {
        Ok(match &self.credentials {
            Some(path) => path.clone(),
            None => Self::directory()?.join("credentials")
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Project {
    #[serde(default)]
    paths: Vec<Mapping>
}

#[derive(Deserialize, Clone)]
#[serde(deny_unknown_fields)]
pub struct Mapping {
    path: PathBuf,
    pub environment: String,
    pub service: String
}

impl Project {
    pub fn resolve() -> BaoResult<Option<Mapping>> {
        let cwd = env::current_dir()?;
        for directory in cwd.ancestors() {
            let file = directory.join("baoenv.toml");
            if !file.is_file() {
                continue;
            }
            let project: Project = toml::from_str(&fs::read_to_string(&file)?)?;
            return Ok(project
                .paths
                .into_iter()
                .filter(|mapping| cwd.starts_with(directory.join(&mapping.path)))
                .max_by_key(|mapping| mapping.path.components().count()));
        }
        Ok(None)
    }
}

pub fn write_private(path: &Path, content: &str) -> BaoResult<()> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| BaoError::File(parent.to_path_buf(), error))?;
    }
    let _ = fs::remove_file(path);
    options
        .open(path)
        .and_then(|mut file| file.write_all(content.as_bytes()))
        .map_err(|error| BaoError::File(path.to_path_buf(), error))
}
